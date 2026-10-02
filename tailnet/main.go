// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0

// fieldnotes-tailnet is the Tailscale that ships inside Field Notes.
//
// It joins the user's tailnet as its own device ("field-notes"), so the computer
// needs no separate Tailscale install, and it answers HTTPS on that device by
// handing each request to the phone portal on loopback. It is Tailscale's own
// tsnet library, the same code the Tailscale app runs, not a reimplementation.
//
// The app starts it, reads one JSON status object per line on stdout, and may
// write commands on stdin:
//
//	logout   sign this device out of Tailscale and forget it
//
// When stdin closes (the app quit or crashed) it shuts down.
//
// What it must never do, and why (the portal's rule 1 depends on these):
//   - Listen anywhere but the tailnet. tsnet listeners only accept tailnet
//     traffic; nothing here opens an OS socket.
//   - Publish to the internet. There is no Funnel, ever; a test in the app
//     checks this file never asks for it.
//   - Send logs to Tailscale. TS_NO_LOGS_NO_SUPPORT is set and logtail is
//     disabled before anything starts.
package main

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"net/http/httputil"
	"net/url"
	"os"
	"strings"
	"sync"
	"time"

	"tailscale.com/client/local"
	"tailscale.com/envknob"
	"tailscale.com/ipn/ipnstate"
	"tailscale.com/logtail"
	"tailscale.com/tsnet"
)

// Status is what the app reads. Field names are the wire format; keep them in
// step with `HelperStatus` in src-tauri/src/tailnet.rs.
type Status struct {
	// Starting, NeedsLogin, NeedsMachineAuth, Running, Stopped, or Error.
	State string `json:"state"`
	// The sign-in page, while State is NeedsLogin.
	AuthURL string `json:"auth_url,omitempty"`
	// The account this device is signed in as, e.g. you@example.com.
	Login string `json:"login,omitempty"`
	// This device's tailnet name, e.g. field-notes.tail1234.ts.net.
	DNSName string `json:"dns_name,omitempty"`
	// The tailnet has MagicDNS and HTTPS certificates turned on.
	HTTPS bool `json:"https"`
	// Answering HTTPS and forwarding to the portal.
	Serving bool   `json:"serving"`
	Error   string `json:"error,omitempty"`
}

var (
	outMu sync.Mutex
	last  string
)

// emit writes a status line, skipping exact repeats so the app isn't flooded
// by a poll that found nothing new.
func emit(s Status) {
	b, _ := json.Marshal(s)
	outMu.Lock()
	defer outMu.Unlock()
	if string(b) == last {
		return
	}
	last = string(b)
	os.Stdout.Write(append(b, '\n'))
}

func main() {
	dir := flag.String("dir", "", "where this device's Tailscale state lives")
	hostname := flag.String("hostname", "field-notes", "the device name on the tailnet")
	target := flag.String("target", "", "the portal on loopback, e.g. 127.0.0.1:8787")
	flag.Parse()

	if *dir == "" || *target == "" {
		fmt.Fprintln(os.Stderr, "usage: fieldnotes-tailnet --dir <state dir> --target 127.0.0.1:<port>")
		os.Exit(2)
	}
	if err := checkLoopback(*target); err != nil {
		emit(Status{State: "Error", Error: err.Error()})
		os.Exit(2)
	}

	// No log uploads, before tsnet reads either setting.
	envknob.SetNoLogsNoSupport()
	logtail.Disable()
	log.SetOutput(io.Discard)

	if err := os.MkdirAll(*dir, 0o700); err != nil {
		emit(Status{State: "Error", Error: "couldn't create the Tailscale folder: " + err.Error()})
		os.Exit(1)
	}

	srv := &tsnet.Server{
		Dir:      *dir,
		Hostname: *hostname,
		Logf:     func(string, ...any) {},
		UserLogf: func(string, ...any) {},
	}
	emit(Status{State: "Starting"})
	if err := srv.Start(); err != nil {
		emit(Status{State: "Error", Error: err.Error()})
		os.Exit(1)
	}
	lc, err := srv.LocalClient()
	if err != nil {
		emit(Status{State: "Error", Error: err.Error()})
		os.Exit(1)
	}

	ctx, cancel := context.WithCancel(context.Background())
	quit := make(chan struct{})
	go readCommands(ctx, lc, quit)

	proxy := newProxy(*target)
	var serving bool

	tick := time.NewTicker(time.Second)
	defer tick.Stop()
	for {
		st, err := lc.Status(ctx)
		if err != nil {
			emit(Status{State: "Error", Error: err.Error()})
		} else {
			s := describe(st)
			if s.State == "Running" && s.HTTPS && !serving {
				if err := serve(srv, proxy); err != nil {
					s.Error = err.Error()
				} else {
					serving = true
				}
			}
			s.Serving = serving
			emit(s)
		}
		select {
		case <-quit:
			cancel()
			srv.Close()
			emit(Status{State: "Stopped"})
			return
		case <-tick.C:
		}
	}
}

// describe turns Tailscale's status into the app's.
func describe(st *ipnstate.Status) Status {
	s := Status{State: st.BackendState}
	if st.BackendState == "NeedsLogin" {
		s.AuthURL = st.AuthURL
	}
	if st.Self != nil {
		s.DNSName = strings.TrimSuffix(st.Self.DNSName, ".")
		if u, ok := st.User[st.Self.UserID]; ok {
			s.Login = u.LoginName
		}
	}
	magic := st.CurrentTailnet != nil && st.CurrentTailnet.MagicDNSEnabled
	s.HTTPS = magic && len(st.CertDomains) > 0
	return s
}

// serve answers HTTPS on this tailnet device only, port 443, with Tailscale's
// own certificate, and hands every request to the portal.
func serve(srv *tsnet.Server, proxy http.Handler) error {
	ln, err := srv.ListenTLS("tcp", ":443")
	if err != nil {
		return err
	}
	go http.Serve(ln, proxy)
	return nil
}

func newProxy(target string) http.Handler {
	u := &url.URL{Scheme: "http", Host: target}
	p := httputil.NewSingleHostReverseProxy(u)
	p.ErrorHandler = func(w http.ResponseWriter, r *http.Request, err error) {
		// The same answer `tailscale serve` gives when the app behind it is
		// down, which the phone already treats as "try again".
		http.Error(w, "Field Notes isn't answering on this computer.", http.StatusBadGateway)
	}
	p.ErrorLog = log.New(io.Discard, "", 0)
	return p
}

// checkLoopback refuses any target but 127.0.0.1: the helper forwards to the
// portal on this computer and nowhere else.
func checkLoopback(target string) error {
	host, _, err := net.SplitHostPort(target)
	if err != nil {
		return fmt.Errorf("bad target %q: %w", target, err)
	}
	if host != "127.0.0.1" {
		return errors.New("the target must be 127.0.0.1")
	}
	return nil
}

func readCommands(ctx context.Context, lc *local.Client, quit chan<- struct{}) {
	defer close(quit)
	sc := bufio.NewScanner(os.Stdin)
	for sc.Scan() {
		switch strings.TrimSpace(sc.Text()) {
		case "logout":
			c, done := context.WithTimeout(ctx, 15*time.Second)
			err := lc.Logout(c)
			done()
			if err != nil {
				emit(Status{State: "Error", Error: "couldn't sign out: " + err.Error()})
				continue
			}
			return
		}
	}
	// stdin closed: the app is gone.
}
