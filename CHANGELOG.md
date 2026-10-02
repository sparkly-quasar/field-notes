<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
# Changelog

User-facing notes, one section per released version. The release workflow reads
the section whose heading matches the tag (`## vX.Y.Z`) and uses it as the
release body — which is also what lands in `latest.json` and what the in-app
"a new version is available" prompt shows. So: write for the person who will
read it inside the app, keep it to what changed, and don't put download links
or Gatekeeper/SmartScreen help here (those are added to the GitHub release page
after publishing — see RELEASING.md).

The heading must be exactly `## vX.Y.Z`, matching the tag. Newest on top.

## v0.17.0

- **Setting up your phone, step by step.** Settings → Devices & server now has a
  checklist that ticks itself off as you go: install Tailscale, sign in, turn on
  HTTPS for your tailnet, turn on device access, publish, and pair your phone.
  The next step always says exactly what to do, with a button that does it or
  takes you there. It re-checks when you come back from Tailscale.
- **First steps** on an empty journal: how to log, that combinations are checked
  as you log, where Get help is, and how to get Field Notes on your phone.
  Dismiss it any time; it goes away by itself once you've logged something.
- **Writing on the phone keeps your intention in view.** With the keyboard up,
  the write-up box now shrinks to fit, so the heading and "You set out to: …"
  stay on screen above it instead of scrolling off the top.

## v0.16.1

- **Run from the menu bar.** For a computer that serves your journal, Settings →
  Server Mode has a new option: **Run from the menu bar instead of the Dock** (on
  Windows and Linux, the system tray instead of the taskbar). Field Notes then
  lives as a small notebook icon next to the clock, closing the window keeps it
  running so your devices can still reach it, and Open and Quit are in the
  icon's menu. When it opens at login, it starts without a window.
- **"Publish to my tailnet" no longer freezes the app.** If Tailscale needs
  Serve or HTTPS approved for your tailnet first, Field Notes now says so and
  gives you a button that opens the approval page. Before, the window froze while
  Tailscale waited for an approval you couldn't see. Any Tailscale step that
  hasn't answered after 15 seconds is now stopped, with a message.
- **The window appears straight away when you open Field Notes.** Your journal
  opens in the background behind "Opening your journal…". If it can't be opened,
  you'll see why instead of a Dock icon that keeps bouncing.

## v0.16.0

**Safety fixes**

- **Warnings now show during a live session on the desktop.** Combination
  warnings and safety prompts appeared on the page underneath the live session
  screen, out of sight. They now appear right under the session's title.
- **The phone's elapsed time keeps up.** "now T+…" on the live session card now
  updates every 30 seconds, and straight away when you unlock the phone.
- **Help stays on screen on the phone** while you scroll, clear of the clock and
  battery. **Get help now** on the desktop starts with "If someone is in danger,
  call your local emergency number now," and only closes when you press Close.

**During a session**

- **When was the last dose?** A live session shows it in large type: "Last: LSD
  100 µg · 1h 12m ago". On the phone, logging the same substance again shows how
  long it's been since the last one.
- **Undo a dose you just logged**, from the "Saved" message on the phone or next
  to "Logged" on the desktop's live session screen.
- **Moments without typing.** Tap how it is (Coming up, Peaking, Calm, Anxious,
  Nauseous, Need water, Coming down) and that's a moment.
- **Dim (red) screen.** True black with red text, easier on your eyes in the
  dark. On the phone it also keeps the screen from locking; turn it on from the
  live session card or when you start a session. On the desktop it's the **Dim
  (red)** button on the live session screen.
- **Ending a session is harder to hit by accident.** It sits apart from logging
  and Help.

**A calmer look**

- **Warmer colours on both the desktop and the phone**, with fewer boxes: the
  journal is a list with dividers rather than cards inside cards, and buttons and
  chips are quieter.
- **The desktop has a sidebar** with the same places as the phone, each with an
  icon: Journal, Stats, Check and Talk, then Settings and **Get help** at the
  bottom. A narrow window folds it back into a row.
- **Substance Log is now part of Stats**, as "Every dose, by substance".
- **Check a combination** on its own, at the top of Check: type two or more
  substances and see what the interaction checker says, without logging anything.
- **Report a bug** moved into Settings.

**Reflection**

- **"What's your intention?"** Starting a session asks, if you want to say. The
  write-up later opens with "You set out to: …", and the rating comes after it.
- **Not every session needs a write-up.** Mark one with **No need** in the
  phone's write-up list, **Doesn't need one** on the entry, or tick **This one
  doesn't need a write-up** when you end it. You can undo it any time.
- **Gentler reminders.** "Add a write-up when you're ready", in quiet text,
  replaces the orange "No write-up yet".

**Privacy and tone**

- **Discreet mode.** Turn on **Offer discreet mode** in Settings on the computer
  that holds your journal. An eye button then appears next to **Help**, there
  and on paired phones. Tap it to swap substance names for stand-ins like
  "Substance K7" and hide entry titles and previews in lists. Each device
  decides when to hide; nothing in the journal changes.
- **Help looks like help.** It has its own warm colour, so it no longer looks
  like Delete or a warning. On the desktop the tab is now **Get help**.
- **Red is only for real danger.** Delete is a plain button until you confirm.
  Warnings say "Known dangerous" or "Use care", and a heavy dose is marked
  "above the usual strong range" in amber.
- **Stats wording:** "days since last session", "Days with a session", "One gap
  so far", and years on the dose chart when the range crosses a year.
- **The desktop uses the same words as the phone:** session, moment, write-up
  and journal note.
- **New type.** Everything you read at a glance (doses, times, buttons) is set
  in Atkinson Hyperlegible, designed by the Braille Institute so that look-alike
  characters such as I, l and 1, or O and 0, can't be mistaken. Your intentions,
  write-ups and moments are set in Literata, a serif made for long reading on
  screens. Both are built into the app; nothing is downloaded.

## v0.14.3

- **Fixed: "All" in the phone's Journal now shows everything.** After picking
  a substance, tapping **All** kept showing only that substance. Now **All**
  clears the substance pick, and so does **Journal notes**, since notes don't
  have doses. **Sessions** and **No write-up** keep it, so you can still ask
  for, say, LSD sessions you haven't written up yet.

## v0.14.2

- **Fixed: Stats now shows only the substance you pick.** Picking a
  substance used to change the dose chart and spacing, but the totals,
  sessions per month, calendar, "taken together" and time of day kept
  showing everything. Now the whole page follows your pick. Choose **All**
  for the overview of every substance.
- **Fixed: one substance logged as "ug" and "µg" showed up twice** (for
  example "LSD · ug" and "LSD · µg"), and a dose could be missing from the
  chart. Micrograms are one unit however they're written, so they're one
  series now.

## v0.14.1

- **Maintenance release.** Internal code tidying with no change to how the app
  works or to your journal. Nothing to do after updating.

## v0.14.0

- **Fixed: the combination check flagged safe-sounding pairs as dangerous.**
  Some substances have one-letter street names in the dose reference (LSD is
  also "L", MDMA "E" and "X", ketamine "K"), and the check treated any name
  that *contained* one of those letters as a match. So "Lithium" matched LSD,
  and LSD with mushrooms came up as "dangerous: high seizure and psychosis
  risk", which is lithium's warning. Matching is now by whole words, which
  removes hundreds of false warnings across the reference and catches some
  real ones the old check missed (for example, "Stimulants" now matches
  substances listed as "Stimulant (mild)"). Warnings about tramadol also now
  apply to O-desmethyltramadol, its active form. If you'd learned to ignore a
  warning that kept coming up, it's worth looking again at what's flagged now.
- **Stats.** A new page, on the desktop and in the phone's Journal tab, laid
  out from what you've logged: each dose over time against the reference's
  light, common and strong ranges; days since you last took something and the
  usual gap between sessions; how often, by week or month; a calendar; what
  you've taken together; and time of day. It describes, it doesn't judge:
  there are no streaks, scores or warnings. **Hide substance names** swaps
  names for letters if you're sharing your screen, and every chart can be
  shown as a table.
- **Changing a time no longer opens a calendar.** Every date and time is now
  two fields, a date and a time. On iPhone and on a Mac, tapping the time
  opens only a time picker.
- **Check for updates yourself.** Settings has a **Check for updates** button
  that tells you whether you're up to date.
- **Update your server from your phone.** When the computer your phone
  connects to has an update waiting, the phone's Today tab says so. If you
  turn on **Let paired phones install Field Notes updates** in Settings on
  that computer, the phone can install it too. The computer restarts and the
  phone reconnects by itself. It's only offered when the computer can come
  back without anyone at it (device access turns on at launch, and an
  encrypted journal can unlock itself), and never while a session is open.
  It's off until you turn it on.
- **Pasting a trip report with T+ times is more reliable.** Times that can't
  be read now fall back to the start of the session instead of being saved as
  text, and a report with no real date is placed on the start you choose.
- **Fixed:** in Settings, the Server Mode checkboxes sat far away from their
  labels.

## v0.13.3

- **Fixed: "Sync journal to server" could copy entries twice.** If the server
  already had an entry — written on both computers, or copied by an earlier
  sync the laptop had lost track of — syncing sent it again. Now every entry
  waiting to be synced is checked against the server's journal first, and one
  that's already there is skipped. The sync tells you how many it skipped.
- **Duplicates are cleaned up.** The first time Field Notes opens after this
  update, it removes entries that are exact copies of another one, keeping the
  original. Every sync also clears exact copies from the server afterwards. Only
  true copies are removed — an entry with anything of its own, even one more
  dose or one changed word, is always kept.
- A sync that lost the connection partway through, and couldn't take its
  half-copied entry back off the server, now removes it at the start of the
  next sync.

## v0.13.2

- **Capsule and pill are now units**, alongside mg, µg, g, ml and tab. A pasted
  log that says "2 capsules" or "1 pill" keeps that unit instead of turning it
  into "tab".
- **The unit follows the substance.** Picking LSD or another lysergamide
  (1P-LSD, AL-LAD, ALD-52 and the rest) switches the unit to **µg**. Picking
  psilocybin mushrooms (or shrooms, truffles) switches it to **g**. Before,
  both stayed on mg, which is a thousandfold off. If you've logged the
  substance before, the unit you used last time still wins.
- **Fixed:** on the desktop, the light/common/strong hint never showed for LSD
  and other microgram substances, because the reference writes "ug" and the
  form writes "µg".

## v0.13.1

- **The phone app now works from your iPhone's Home Screen.** iPhone keeps a
  web app saved to the Home Screen separate from Safari, so a phone paired in
  Safari opened the saved app to a "Not paired" screen with nothing to do. Now
  the Home Screen app lets you paste your pairing link to pair it once. In
  Safari, a card offers **Copy link for the Home Screen app** so you have the
  link ready. It's a key to your journal, so paste it straight in and don't
  keep it anywhere else.

## v0.13.0

- **Paste a trip log, on the phone or the desktop.** Copy a log from anywhere —
  a notes app, a message — and paste it in (**＋ → Paste a trip log** on the
  phone, **Paste a trip log** in the Journal on the desktop). Lines like
  `8:43am - 35mg mesc`, `22:10 nothing yet` or `T+1:30 peak` become the
  timeline, with doses recognised — street names included ("mesc", "molly",
  "ket") — and times past midnight kept in order. You see every line before
  anything is saved, and can fix any of it. It's read on the device, with no
  AI involved, and every dose gets the usual interaction check.
- **A phone app that's easier to find your way around.** Five tabs — Today,
  Journal, **＋**, Check, Talk. Everything new starts from **＋**: log a dose,
  start a session, log a past session, paste a log, or write a note.
- **Log a past session properly.** Say when it started, then add what you took
  and what happened — each new line starts just after the last, so there's
  almost no fiddling with times — and finish with when it ended, a rating and a
  write-up.
- **Reading back is easier.** The journal is grouped by month, searchable, and
  filterable by substance or by entries still waiting for a write-up. An entry
  reads like a page: the facts, the timeline with T+ times, intention, setting,
  write-up.
- **Editing happens where your thumb is.** Tapping any line opens its editor at
  the bottom of the screen, not somewhere below the fold. Doses can now be
  renamed, not just re-timed.
- **Help on every screen.** A Help button in the corner shows emergency and
  support lines, even when the phone can't reach your server. And when a dose
  makes a session's combination dangerous, the phone now says so the way the
  desktop does.
- **Harder to lose something by accident.** Deleting waits a few seconds with
  **Undo** instead of asking "are you sure?", and Delete sits well away from
  Save.
- **Fixed:** amounts typed with a comma ("1,5") were silently dropped — they're
  now saved as 1.5. Phone numbers in the emergency list dialled the wrong number
  when the entry listed more than one. The same interaction warning could show
  twice. The phone's layout now clears the notch and the home indicator, and
  follows your light/dark setting.

## v0.12.0

- **Keep your journal on one computer, and use it from all of them.** A
  computer that stays on — a desktop at home — can now be your Field Notes
  server. Your laptop connects to it over Tailscale and reads and writes the same
  journal, and your phone pairs with it the way it always has. Settings →
  **Devices & server** on the server, **Use another computer as your server**
  on the laptop.
- **Bring your existing journal with you.** Once the laptop is connected,
  **Sync journal to server** copies the entries it already had — doses,
  timelines, write-ups and any substances you added — to the server. They stay
  on the laptop too, and syncing again only copies what's new.
- **Every device has its own key, and pairings survive a restart.** Pair each
  device with a name ("Phone", "Laptop"), see when it was last active, and
  un-pair one without touching the others. Restarting Field Notes no longer
  means re-scanning the QR code.
- **Server Mode: a server that looks after itself.** Optional, for the computer
  that is the server: turn device access on whenever Field Notes opens (on the same tailnet
  address as before, so nothing needs re-pairing), open Field Notes at login,
  and — if your journal is encrypted — save the password in the system keychain
  so it unlocks on its own after a reboot. Settings explains the trade-off
  before you choose it.
- **Losing the connection doesn't lose a dose.** If the laptop can't reach the
  server, new sessions, doses, timeline notes and ending a session save on the
  laptop and are sent, in order, when it's back — the header says how many are
  waiting. The interaction checker and crisis resources keep working on the
  laptop meanwhile, against the session as it stands. Editing and deleting wait
  for the connection, so two copies can never disagree.

## v0.11.5

- **Logging a dose is now the shortest path, on the desktop too.** The Journal
  header leads with **+ Dose**: a substance, an amount, and roughly when. That's
  the whole entry — no session to start or end, nothing to write up. The phone's
  version of the same thing got the same treatment.
- **Fewer taps to get there.** Substances you've logged recently are one tap.
  So are the times you actually reach for — **Now**, **1h ago**, **3h ago**,
  **Last night** — and they fill the date field rather than replacing it, so you
  can always see the exact time being saved and nudge it. The unit and route are
  remembered per substance, so cannabis stops asking you to change "mg" to "g"
  every time.
- **Log it now, write it up later.** After saving, **Add notes to it** opens the
  entry with the write-up ready to type into, and entries that don't have one yet
  are marked "no notes yet" in the journal, so nothing quietly stays unfinished.
  Everything else about the entry — times, rating, more doses — can still be
  edited afterwards, from either screen.
- **A second substance goes in the same entry.** **Log another into it** adds to
  what you just logged rather than starting a separate entry. That's how the
  evening reads back, and it means the interaction checker sees the two together.
- **The checker doesn't go quiet on a quick log.** An entry with a single dose has
  nothing to compare against, so anything else you logged within twelve hours
  either side is checked against it too.

## v0.11.4

- **Log something you took, without writing a trip report.** The phone's Now
  screen now opens with a one-shot form: substance, amount, route, and when you
  took it. That's the whole entry — no session to start, none to remember to
  end, nothing to write up. The time defaults to now, so it's four taps for
  something you just took, and you can set it back to any day you're catching up
  on. It lands in your journal named after the substance, and the interaction
  checker still runs against anything else you logged around the same time.
- **Fix a past entry from your phone.** Entries in the Journal used to be
  read-only there — you could read one back and rename it, and that was all.
  Now you can tap any dose or note in it to correct it (including the time,
  which was previously desktop-only), add a dose or a note you forgot at the
  time it happened, and open **Edit entry** for the title, the start and end
  times, the rating, and the write-up — or delete the entry. So a quick log can
  become a full entry the next morning, if you want it to.
- **The phone's timeline reads in order.** Doses and notes were listed in two
  groups; they're now interleaved by time, like the desktop's.

## v0.11.3

- **Sessions name themselves.** Starting a session from your phone gave you
  nowhere to type a title, so it landed in the journal as "Untitled" and usually
  stayed that way. Now an untitled session takes the name of the first substance
  you log into it — so a session you started one-handed in the dark shows up as
  "ketamine" rather than "Untitled". Only the first dose names it, a title you
  typed yourself is never overwritten, and you can still rename anything at any
  time by tapping its title on the phone or editing it on the desktop.
- **A title field when you start a session on the phone.** Optional — leave it
  blank and the naming above takes over.

## v0.11.2

- **Phone access no longer takes over a port another app is using.** If something
  else on your computer is already published to your tailnet — a chat interface, a
  media server, anything using `tailscale serve` — Field Notes now publishes
  alongside it on a free port instead of quietly replacing it. Previously it
  claimed the standard HTTPS port whatever was already there, which took the other
  service off your tailnet without saying so.
- **The pairing QR code now points where the portal actually is.** When Field Notes
  publishes on a different port, the QR code and the address shown on screen carry
  that port. Before, they always showed the standard address, so on a machine
  running other services the QR could send your phone to the wrong app entirely.
- **Turning phone access off only turns off Field Notes.** It now retracts its own
  connection and nothing else. Before, it switched off the standard port
  unconditionally, which could take down an unrelated service while leaving the
  journal published.

These only affect computers running other tailnet services alongside Field Notes;
if it's the only one, nothing changes.

## v0.11.1

- **Top-bar tidy-up.** The menu is reordered into a more natural flow — Journal,
  Substance Log, Substance Directory, Companion, Settings, then Emergency
  Resources and Report a bug at the end. The old "Substances" reference tab is now
  labelled **Substance Directory** so it's clearer what it is, and the "Report a
  bug" button lost its emoji so it sits cleanly alongside the rest.

## v0.11.0

- **Logging a past experience is now a first-class option.** The "+ Session" form
  has a **"This already happened"** checkbox — tick it and you get an end-time
  field, and the session is saved as a finished trip (not an ongoing one) rather
  than having to start a live session and backdate it. Doses you add to a
  finished session default to when it happened instead of "now", so writing up an
  old trip doesn't mean fixing every timestamp.
- **A "Report a bug" button now lives on the top bar**, so filing a bug or feature
  request is one click from anywhere instead of buried in Settings. It opens the
  same prefilled GitHub issue in your browser — nothing leaves your journal.

## v0.10.2

- **This update prompt now tells you what changed.** Until now it only showed a
  version number; from here on it shows the actual list of what's new — the notes
  you're reading. (You're seeing this because you updated *to* the version that
  added it, so this is the first time it appears.)
- **The app now checks for updates on its own while it's open**, every few hours,
  so a machine left running for days still finds out about a new version without
  being restarted. The check is silent — you'll only ever notice it when there's
  actually something new.

## v0.10.1

- **You can see when your phone has paired.** Setting up phone access used to
  give no feedback — you showed the QR code, scanned it, and then guessed. Now a
  green "Paired successfully" light appears the moment your phone first connects.
  It confirms a phone has paired since you turned phone access on (not that one
  is connected this second), and resets when you turn phone access off. Phone
  access is unchanged otherwise: off by default, tailnet-only, code on every
  request.

## v0.10.0

- **The Companion stays responsive.** It no longer locks the window while it
  thinks (worst on the first message, when the model loads), and the model
  pre-loads when you open the Companion so the first reply comes back at normal
  speed. Same fix for Import from text.
- **It's honest about your hardware.** The app checks whether your machine can
  run the model you picked and says so plainly — before a multi-gigabyte
  download, and again using real reply speed once you've chatted.
- **Import from text actually works now.** It reads any format, pulls out every
  dose, and uses each substance's standard name so the interaction checker
  recognises it. The screen shows everything it found for review.
- **A first-run choice for the Companion**, asked once on a fresh install.
- **Send feedback in one click** — a Settings card opens a prefilled GitHub
  issue in your browser. Nothing is sent from the app itself.
