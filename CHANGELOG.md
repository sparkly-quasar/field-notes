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

## v0.29.3

- **Picking a substance suggestion no longer taps what's under it.** Choosing a
  name from the list could also press the button or field underneath it once
  the list closed.

## v0.29.2

- **"Likely still active" shows only what's active now.** The chart and list
  no longer bring back everything from the last two days. A substance counts
  while its effects are likely still felt; after-effects don't count, and drinks
  count while some are still being processed. The chart is centred on now, and
  the amounts and tier count only the doses still active, not yesterday's.
- **Device access turns back on, however often you switch it.** Turning it off
  left its connection point held open. After enough off-and-on cycles it
  refused to start again until the app was restarted.

## v0.29.1

- **Times use the 12-hour clock** (9:30 pm, not 21:30) everywhere the app
  shows one: entries, trip reports, Stats, the active-arcs chart, PDFs and the
  phone.
- **Prefer 24-hour?** Settings → **Clock** switches it, on each computer and
  phone separately. The time pickers you type into follow your system's clock
  setting either way.

## v0.29.0

- **Active arcs.** A chart shows what each substance is
  likely doing now, drawn from the dose reference's own timings, with a line for
  now. Each substance is one line: a top-up adds into it, so the line shows the
  dose you're carrying, and its height follows the reference's dose tiers.
  Under it, "Likely still active" gives each one's phase (coming up, peaking,
  winding down, after-effects) and when the next stage usually arrives. Never a
  countdown, and a line ending isn't an all-clear.
- **Tap a substance in the key** to see its timing range on the chart and what
  a redose means for it. For MDMA and stimulants, a second dose raises levels in
  your body more than it raises how it feels; for psychedelics it adds with
  diminishing returns; edibles arrive late, so a "didn't work" redose stacks.
- **"Likely still active" on Today, even without a live trip report.** Anything
  you've logged in the last two days that may still be active shows there, from
  any entry, with the chart a tap away. In a live trip report it's on the live
  card. **Check** lists it too, with a button that puts it into the combination
  check, ready for whatever you're thinking of adding.
- **Finished trip reports show their arcs,** with your moments on them: how the
  doses likely played out, next to how it actually felt.
- **Drinks stack and clear on their own scale,** at about one standard drink an
  hour. That's a rule of thumb, and many people clear more slowly.

## v0.28.2

- **The combination check knows a substance however you spelled it.** A dose
  logged as "3meo", "3meopcp" or "3-MeO" is now checked as 3-MeO-PCP, including
  entries you logged before this update. Until now, a name written another way
  could miss warnings, such as the one for a dissociative with alcohol.

## v0.28.1

- **Every substance shows up as you type.** A name you've typed in full stays in
  the suggestions, so 3-MeO-PCP no longer seems to vanish beside 3-MeO-PCPr, and
  LSD no longer disappears once you've typed it.
- **Hyphens and spaces don't matter.** "3meopcp", "3 meo pcp" or "2cb" finds the
  right substance, and is saved as it.

## v0.28.0

- **Unlock your journal with a PIN.** If you share someone's Field Notes server,
  you can set a 4 to 12 digit PIN in Settings on your phone and use it instead
  of your password when your journal locks. It works only on that device, turns
  off after five wrong tries, and turns off on all your devices if you change
  your password. Your password stays the real key: the server never keeps it.

## v0.27.3

- **San Pedro and peyote count as psychedelics,** however they're written
  ("San Pedro powder", "Peruvian torch", "huachuma"), in Stats and in the
  combination check.

## v0.27.2

- **A bigger substance box on phones.** It now matches the other fields, and
  tapping it no longer zooms the page in.

## v0.27.1

- **No more "stimulating" or "sedating" note on drinks.** Alcohol no longer
  carries a note on what that number of drinks tends to do. Its combination
  warnings and dependence notes are unchanged.

## v0.27.0

- **Tag someone in a dose.** When two people share a server and have both
  allowed it (Settings on your phone, under Tagging), either can tag the other
  in a dose, right after logging it or later from the dose itself. The other
  person sees it on Today, can change the amount, time or anything else, and
  adds it to their own journal, or declines. Only the dose travels: never the
  rest of your entry, and neither of you can see the other's journal. You see
  whether it was added or declined, nothing more. Waiting tags are kept only
  while the server is running. On phones for now.
- **Name suggestions as you type.** Substance fields suggest names from your
  own substances and the dose reference, street names included ("acid" offers
  LSD), on the phone and on the computer.

## v0.26.1

- **A spacing note that fits kratom.** Kratom's Stats now explains its own
  tolerance and dependence, and when its risks rise (mixing with alcohol,
  benzodiazepines or other opioids, and 7-OH products), instead of the
  general opioid note.

## v0.26.0

- **Stats or Talk in the phone's bottom bar.** In Settings on your phone,
  under Bottom bar, choose whether the last button opens Talk or goes straight
  to Stats. The choice is kept on that phone only.
- **Shorter wording.** The question about how you take a substance now just
  says "As needed", and Stats no longer shows the note about substances taken
  as needed or as routine.

## v0.25.0

- **Stats pays attention to the amount, not just the dose.** Each dose is
  shown as what it was logged as against the dose reference's ranges ("logged
  as a strong dose"), never as what you received, since potency is unknown.
  The calendar shades each day by its strongest dose, and microdoses are
  recognised on their own: an outline on the calendar, a lighter part of each
  bar, and stretches of microdosing noted below.
- **Kratom in mg and g is one series now.** Amounts in different mass units
  (mg, g, µg) are combined, and the reference's ranges show even when you log
  in a different unit from the reference's.
- **Say what form a dose was in.** Mushrooms can be dried, fresh, powdered or
  an edible, and kratom leaf powder, extract or a 7-OH product. Fresh
  mushrooms count as about a tenth of their weight in dried, and an edible by
  your own estimate in grams or mg of psilocybin. With no form chosen, dried
  is assumed. Capsules are counted by how much is in each, and you can save
  your usual capsules so you only enter that once.
- **What an amount tends to do.** For a few substances whose effects change a
  lot with the amount (kratom, DXM, diphenhydramine, ketamine), a dose
  carries a short note on what that amount usually does, and the combination
  check takes it into account. Ibogaine always carries a heart-rhythm caution.
  These only ever add to a warning; they never soften one.
- **Routine and as-needed doses.** If you take something at the same time most
  days, Field Notes may ask once whether it's part of your routine. For
  benzodiazepines, sleeping pills, gabapentinoids and kratom it may ask
  whether you take it as needed. Either way the doses stay in your journal,
  the combination checks and that substance's Stats; they just aren't counted
  as experiences. You can change this on the substance's Stats page. It never
  asks about alcohol, GHB or opioids.
- **Worth knowing.** When a pattern shows up that's linked to dependence or
  withdrawal (daily GHB, daily drinking, regular benzodiazepine or kratom use,
  and so on), a short note explains what it can mean. It shows once after the
  dose, then stays on that substance's Stats page. No scores or streaks.
- **Doses near bedtime.** Add your usual bedtime under Time of day to see which
  doses were taken within one half-life of it. You can also choose to be asked
  how you slept each morning, and compare nights after a dose near bedtime
  with other nights. Both are off until you turn them on.

## v0.24.0

- **Tap a chart in Stats to see what's behind it.** Tap a day on the calendar
  to see that day's experiences, a bar to see that week's or month's, or an
  hour under Time of day to see the experiences with a dose then. Each one
  opens straight into the journal. The Trends charts work the same way,
  including the dots on Time between and Amount per experience. On Amount per
  experience, a tap now shows the experience first instead of leaving Stats
  straight away.

## v0.23.2

- **Drinks logged by name count as alcohol.** Logging "Beer", "wine", "vodka",
  a "shot" and so on is now treated as alcohol, so mixing it with a benzo,
  opioid or other depressant gets a warning, and it shows up under
  depressants in Stats.

## v0.23.1

- **What your phone saves offline is better protected.** The first time
  something is saved on your phone to send later, the app asks the browser to
  keep its storage even if the phone runs low on space. On Android with the app
  on your home screen, this is usually granted without asking.

## v0.23.0

- **Gentler warnings once a dose is wearing off.** If two substances only
  overlap after one of them is past its peak, the warning now says so ("past
  its peak by the time these overlapped… it is fading, not gone") above the
  usual advice. Some combinations always get the full warning: anything rated
  dangerous, anything with an MAOI, two serotonergic drugs, and a stimulant
  with an opioid, benzo or GHB-type sedative, where the stimulant wearing off
  is the risk.
- **Quick logs check timing.** A quick log is now checked against each dose at
  the time it was actually taken, not against everything from the last 12
  hours.
- **Clearer GHB warnings.** Stimulant warnings about GHB or GBL no longer call
  it "the opiate".
- **Shorter Trends intro.**

## v0.22.2

- **Save or share an experience as a PDF.** To send a trip report to someone:
  on your phone, open the entry, tap ⋯ and choose **Share as PDF** (straight
  to Messages, Mail or AirDrop where your phone offers it, or download it); on
  the computer, **Save as PDF…** on the entry. The title, times and doses are
  always in it, and you choose whether to include intention and setting,
  moments, the write-up and the rating. The timeline shows each dose and
  moment with its time and T+ from the first dose. It uses real names, even in
  discreet mode, and it works on the phone without a connection too.

## v0.22.1

- **Settings on your phone.** The gear next to Help now appears on the owner's
  phone too, not just for other people on the server. In it:
  - **Pair and un-pair your devices from the phone.** It asks for your
    journal's password first, or, if your journal isn't encrypted, a phone PIN
    you set in Settings on your computer (Devices & server). After 5 wrong
    tries it waits 15 minutes. Someone holding your phone can see your devices
    but can't add or remove one without it.
  - **Discreet mode** for this phone.
  - **What the phone keeps** for when your computer is away, with a button to
    clear it. Entries still waiting to be sent are never cleared.
  - **Updates:** which version your computer is on, and installing a new one.
  - **The Home Screen link**, to pair the Home Screen app.
- **Street names are saved as the substance.** Logging "acid" saves LSD, and
  "dexamp" saves Dextroamphetamine, with a note under the field and a tap to
  keep your own wording. Only whole names count, never one- or two-letter ones
  like "L" or "K", and your own substances always come first.
- **Stats counts both spellings as one.** Doses you logged before under a
  street name now join the same history, Trends included. Nothing in your
  journal is renamed.
- **Only the experiences you'd reflect on ask for a write-up.** An ended
  experience asks when it has a psychedelic, an empathogen like MDMA, a
  dissociative like ketamine, or something the app can't place. An entry with
  only alcohol, a stimulant like dexamp, or a sedative like 14b no longer
  shows up under "To write up", unless it's part of a trip that also has a
  psychedelic. You can still write up anything, and "Doesn't need one" still
  works everywhere. This applies to entries you've already logged.

## v0.22.0

- **The phone works when your computer can't be reached.** If the computer is
  asleep, off, or out of reach, the phone page still opens, and:
  - **Check and Look up answer on the phone**, using the same combination
    checker your computer runs, over a copy of the dose reference the phone
    saves while it's connected. Anything it can't check (a substance you added
    yourself, or one not in the reference) is named, never passed over quietly.
  - **New doses, moments and entries save on the phone** and are sent to your
    computer, in order, as soon as it's back. A dose logged offline is still
    checked against what else you've taken. A note at the top says how many are
    waiting; anything your computer turns down stays there to send again or
    discard.
  - Editing, Stats and Talk wait for the computer, and say so.
- **What the phone keeps:** entries waiting to be sent (deleted as your
  computer confirms each one), the experience in progress, and the last day's
  entries by substance and time, for the offline check. Your own substance
  catalogue stays on your computer.
- Opening the server's address without `/m` now goes to the phone page.

## v0.21.7

- **"Experience", not "session".** The app now talks about experiences
  throughout, including Stats.
- **Trends, under Stats.** Compares the last stretch of time with the one before
  and lays out what changed, each with a small chart: how often (the headline),
  time between experiences of one substance, amount per experience, redosing
  and combinations. A trend only appears once each period has at least three
  experiences. It describes, never judges. For psychedelics, a rising amount
  carries a note that starting low and working up gradually is a common way to
  work with them.
- **MDMA with a psychedelic is its own note.** MDMA acts as an empathogen
  rather than a plain stimulant and tends to soften a psychedelic's anxiety, so
  the advice for that pairing now says so, while still covering heart rate,
  temperature and serotonin.
- **5-MeO-DMT dose ranges corrected.** Oral and the duplicate inhaled route are
  gone (it isn't orally active). Smoked and insufflated ranges come from
  Erowid, and there are intramuscular ranges. The lookup says where the figures
  come from.
- **Log alcohol in drinks and cannabis in hits.** One drink is one standard
  drink, with quick picks for a beer, a glass of wine and a shot; a mixed drink
  counts its shots. Smoked or vaped cannabis defaults to hits, which are never
  compared against mg ranges. Pasted trip logs understand "2 beers", "a shot of
  tequila" and "3 hits".
- **Phone: the current tab is easier to see.** The active tab sits on a tinted
  pill in the bottom bar.
- **Steadier update check from the phone.** A failed update check can no longer
  leave the phone stuck on "checking".

## v0.21.6

- **Combination warnings are quieter and more useful.** Each one is a single
  line with an icon and the substances involved. Tap it for what the risk is and
  what lowers it: spacing, amounts, cooling down, who to have with you.
  Repeated warnings with the same message show once, naming every pair.
- **Warnings look at timing.** Two doses only count as a combination if both
  were active at the same time, by how long each lasts.
- **No more "get help now" over what you've logged.** A risky combination is a
  note to read, live or past, never an emergency banner: what was taken can't
  tell anyone how you're doing. If something you write in Talk sounds like it
  might be urgent, the app offers the people who can help, calmly, and Help is
  always one tap away.
- **MDMA with mephedrone is now "use care"** rather than "dangerous": the risk
  depends heavily on the amounts and how close together they're taken.
- **Fewer false matches, one real one added.** Warnings about the 5-MeO
  tryptamines no longer fire for DMT. 1,4-butanediol now gets GHB's
  interaction warnings (it becomes GHB in the body), and rilmazafone is
  recognised as a benzodiazepine.

## v0.21.5

- **Pasting a trip log gets the times right when it has no am/pm.** A log
  written as `5:30 … 12:30, 1:15` is no longer read as starting in the morning
  with a twelve-hour gap after noon. The preview asks whether the first line was
  am or pm (guessing from a time next to the date, if there is one).
- **A line with several doses becomes several doses.** `150mg MDMA, zofran` and
  `100ug LSD + 5g mushrooms` are one dose each, so nothing is dropped and each
  one gets its interaction check.
- **Other people's doses are kept, as theirs.** Lines like `40mg DMT - Sam`,
  `Alex 40mg dmt` or `2C-D (11 Alex 6 Sam)` are recognised. The preview asks
  which one is you; yours are logged as doses, everyone else's as notes on the
  timeline.
- **What you wrote after the log becomes the write-up**, instead of being stuck
  onto the last line.
- **Routes are filled in more sensibly.** "rectal", "IM" and the like no longer
  end up in the substance's name. A dose with no route written takes the route
  of the same substance earlier in the log, then the one you usually log it
  with, and DMT and ketamine are no longer assumed to be swallowed (DMT stays
  oral if the log mentions an MAOI).
- **More names recognised:** 4mmc / 4-MMC / meph / m-cat (mephedrone), 14b /
  1,4b (1,4-butanediol) and dexamp (dextroamphetamine). `11:30ish` reads as a
  time.
- **End trip report**, and **Start a trip report** on the desktop's empty
  journal.

## v0.21.4

- **A calmer Today screen on the phone.** Today now has one big **+ Log a
  dose** button instead of a stack of four. Starting a live trip report,
  logging a past session, pasting a trip log and writing a journal note are
  all under the **＋** button, which now says **New** so it's easier to spot.
  With an empty journal, Today also offers **Start a live trip report**.
- **"Live session" is now "live trip report"**, on the phone and the desktop,
  and **End session** is now **End trip report**.
- **＋ New offers Add a moment** while a live trip report is going.
- Recent on Today shows six entries instead of four. The Home Screen tip moved
  below it.

## v0.21.3

- **Settings is split into sections, so less scrolling.** A row at the top
  switches between **General** (updates, the Companion, discreet mode, the
  startup disclaimer), **Devices & sync**, **Data & privacy** (encryption,
  backup, Obsidian vault sync, erase & uninstall) and **Feedback**. The server
  button in the header and "Set up your phone" open straight to Devices & sync.

## v0.21.2

- **Settings puts what you reach for first at the top.** Check for updates comes
  first, then using another computer as your server, then Devices & server
  (paired devices, people, Server Mode). The Companion switch moved down, after
  Obsidian vault sync.

## v0.21.1

- **Install a server update from your phone even when someone else's journal is
  unlocked.** Before, the phone just said to install it at the computer. Now it
  says whose journal the restart would lock and offers **Install anyway**. They
  unlock it again from their phone afterwards. It still won't let you if
  someone else has a session going and has used Field Notes in the last two
  hours: wait until they've ended it. A session nobody has touched for longer
  than that doesn't hold the update up.

## v0.21.0

- **Phone access without installing Tailscale on your computer.** Tailscale is
  now built into Field Notes. In Settings → Devices & server, press **Connect**
  and sign in to Tailscale in your browser (with Google, Apple, Microsoft or
  GitHub; signing in creates an account if you don't have one). Then install
  Tailscale on your phone, sign in with the same account, and scan the code.
  Your computer shows up in Tailscale as "field-notes". Tailscale can see which
  of your devices are connected, never what's in your journal, and Field Notes
  turns off Tailscale's log uploads.
- **Already using the Tailscale app on this computer?** Nothing changes: your
  phone keeps working at the same address. You can switch to the built-in one
  in Settings whenever you like (your phone then needs its code scanned again).
- **Clearer messages on the phone when it can't connect.** It now says what to
  check: that Tailscale is on and signed in with the same account as your
  computer, or that Field Notes is open on the computer.
- **The Companion is now optional and off on new installs.** Turn it on in
  Settings if you want it; it needs a separate download. The interaction
  checker, dose reference and crisis resources never used it and work the same.
  If you already use the Companion, it stays on.

## v0.20.2

- **Spacing guidance in Stats.** The Spacing card for psychedelics,
  entactogens, dissociatives, cannabinoids, opioids and depressants now shows
  what harm-reduction sources usually say about time between uses, with the
  reason. For example: psychedelic tolerance mostly fades in 3 to 7 days; a
  month or more between MDMA sessions, with three months widely recommended;
  after a break from opioids, start lower. It shows on a substance's card too.
  It's reference text only, and never compares your own gaps to it. In
  discreet mode it's hidden, since it names substances.
- **Pasted text that's full of %20 now reads normally.** Some apps copy text the
  way it looks inside a link ("I%E2%80%99ve%20been..."). Pasting a trip log
  turns that back into ordinary words, even when only part of the paste was like
  that. A log that just mentions "50%" is left as you wrote it.
- **The date at the top of a log is used.** If your log starts with a line like
  `Date: 02-28-2026 12:30pm` or `Saturday, February 28, 2026`, the import picks
  that day for you instead of guessing today or yesterday, and says it did. You
  can still change it before saving.
- **A time on its own line, with the words underneath, now works.** Writing
  `12:30pm` on one line and `20mg 2C-D after breakfast` on the next is read as a
  dose at 12:30, the same as if it were all on one line.

## v0.20.1

- **Pasted text that's full of %20 now reads normally.** Some apps copy text the
  way it looks inside a link ("I%E2%80%99ve%20been..."). Pasting a trip log
  turns that back into ordinary words, even when only part of the paste was like
  that. A log that just mentions "50%" is left as you wrote it.
- **The date at the top of a log is used.** If your log starts with a line like
  `Date: 02-28-2026 12:30pm` or `Saturday, February 28, 2026`, the import picks
  that day for you instead of guessing today or yesterday, and says it did. You
  can still change it before saving.
- **A time on its own line, with the words underneath, now works.** Writing
  `12:30pm` on one line and `20mg 2C-D after breakfast` on the next is read as a
  dose at 12:30, the same as if it were all on one line.

## v0.20.0

- **More than one person on one server.** Someone you live with can now use
  your Field Notes server with a journal of their own. In Settings → Devices &
  server → People, add them by name and let them scan the pairing code. Their
  phone asks them to choose a password, and their journal is encrypted with it.
  Nobody else who uses the server can see it, and you can't read it either: the
  People list shows their name, how many devices they have and whether their
  journal is locked, never their entries.
- **Their password, their choice.** Your server never stores their password
  unless they turn on "Keep my journal unlocked on this server" on their own
  phone. Otherwise a restart locks their journal until they type it again.
  Help and the combination checker still work while it's locked. A forgotten
  password can't be reset by anyone, so the phone says so when they choose it.
- **Their own backups and password.** From their phone's settings they can
  download a backup of their journal, still locked with their password, and
  change that password. A week after choosing it, their phone reminds them
  once that a backup is the only way back if they forget it.
- **Their own devices.** From their phone's settings they can pair another
  device of theirs or un-pair one. You can also pair a replacement for them
  from the desk if they lose a phone, or remove them, which deletes their
  journal after you type their name.
- **Nothing changes if it's just you.** Your journal, your devices and your
  phone work exactly as before. Only you can install updates from a phone, and
  an install waits if someone's journal would lock because of it.

## v0.19.0

- **Stats by drug family.** A new row above the substances lets you pick
  Psychedelics, Entactogens, Dissociatives, Stimulants, Depressants, Opioids or
  Cannabinoids, and the whole page narrows to it: days since the last one of
  any kind, sessions and doses, spacing across the family (LSD then mushrooms
  nine days later counts as nine days), which substances it was, and the
  calendar, charts and combinations for just that family.
- **How substances are sorted.** Your own catalogue comes first, so you can
  always correct one. Then the DoseWiki reference, using its main classes only,
  so MDMA counts as an entactogen and a stimulant but not as a psychedelic.
  Then Field Notes' own name matching, for things the reference leaves out,
  like alcohol, GHB and nitrous. Anything it can't place goes under Other.

## v0.18.1

- **A few desktop buttons had no style.** "Pair", "Stop publishing to my
  tailnet", "Show QR code", "Copy link", "Check for updates" and others in
  Settings showed as plain white boxes after the redesign. They're outlined
  pills now, like the rest.
- **Less explaining on the phone's Today screen.** The line under "Log
  something you took" is gone; the buttons say it.

## v0.18.0

- **Pasting a trip log from some apps showed "%20" everywhere.** Text copied as
  part of a link now turns back into normal words when you paste it or press
  Read it. A log that just mentions a percentage is left alone.
- **"Couldn't read the clipboard" is no longer a red error.** When a browser
  won't let Field Notes read the clipboard, it now says how to paste into the
  box instead, in ordinary text.
- **A cleaner, quieter look.** On the desktop, content now sits directly on the
  page instead of inside cards, with a thin bar at the top of each view for its
  actions (Paste a log, Import, Note, Session, + Dose). The sidebar is smaller
  and quieter, and the journal is grouped by month with the date on the left.
  On the phone, sections are divided by lines instead of floating as cards, and
  headings are smaller. The live session screen, Help, + Dose and + Moment
  keep their size.
- **Buttons look like buttons.** Every button is now a rounded pill, and cards,
  sheets and text boxes stay rectangles, so anything you can tap is recognisable
  by its shape, even on the dim red screen. Main actions are filled; the rest
  are outlined instead of a block of colour that blended into the card.
- **+ Dose leads.** In a live session, + Dose is the one filled button and
  + Moment is outlined beside it, the same size and in the same place. Both are
  a little smaller to look at, but take a tap just as easily as before.
- **A softer accent colour**, a muted violet that sits with the warm colours
  instead of the old cool blue. Green, amber, red and Help's cream keep their
  meanings.
- **"Open session ›"** on the live card is now a simple link, so the card shows
  the session first and the buttons second.

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
- **A real Home Screen icon.** Field Notes added to a phone's Home Screen now
  shows the notebook icon instead of a letter "F". If you added it before, remove
  it and add it again to pick up the icon.

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
