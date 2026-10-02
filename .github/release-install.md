<!-- install-guide -->
<!--
  Appended to every GitHub release page by the `install-guide` job in
  .github/workflows/release.yml, after the CHANGELOG section. It is NOT in
  latest.json, so the in-app update prompt never shows it.
  @VERSION@ becomes e.g. 0.21.6 and @TAG@ becomes v0.21.6.
-->

---

## How to install

**Already have Field Notes?** You don't need to download anything. The app offers
this update itself next time you open it (**Install & restart**), and your
journal stays exactly where it is.

**New to Field Notes?** Pick your computer below and follow the steps.

### Windows

1. Download **[Field.Notes_@VERSION@_x64-setup.exe](https://github.com/sparkly-quasar/field-notes/releases/download/@TAG@/Field.Notes_@VERSION@_x64-setup.exe)**.
2. Open the downloaded file.
3. Windows shows **"Windows protected your PC"** because the installer isn't
   code-signed yet. Click **More info**, then **Run anyway**.
4. Follow the installer, then open **Field Notes** from the Start menu.

<sub>Prefer an MSI? [Field.Notes_@VERSION@_x64_en-US.msi](https://github.com/sparkly-quasar/field-notes/releases/download/@TAG@/Field.Notes_@VERSION@_x64_en-US.msi)</sub>

### macOS (Apple Silicon and Intel)

1. Download **[Field.Notes_@VERSION@_universal.dmg](https://github.com/sparkly-quasar/field-notes/releases/download/@TAG@/Field.Notes_@VERSION@_universal.dmg)**.
2. Open the `.dmg` and drag **Field Notes** into **Applications**.
3. Open Field Notes from Applications. macOS blocks it the first time because
   the app isn't notarized yet. Click **Done** (not *Move to Trash*).
4. Open **System Settings → Privacy & Security**, scroll down, and click
   **Open Anyway** next to *"Field Notes" was blocked*. Confirm with your
   password or Touch ID.
5. Open Field Notes again and click **Open**. You only do this once.

<sub>Comfortable with Terminal? Step 4 can instead be
`xattr -dr com.apple.quarantine "/Applications/Field Notes.app"`.
If the icon keeps bouncing and no window opens, select Field Notes in
Applications, press **Cmd+I** and untick **Locked**.</sub>

### Linux

**AppImage (any distribution)**

1. Download **[Field.Notes_@VERSION@_amd64.AppImage](https://github.com/sparkly-quasar/field-notes/releases/download/@TAG@/Field.Notes_@VERSION@_amd64.AppImage)**.
2. Make it executable: `chmod +x Field.Notes_@VERSION@_amd64.AppImage`
   (or right-click → Properties → *Allow executing as program*).
3. Double-click it, or run `./Field.Notes_@VERSION@_amd64.AppImage`.

**Debian / Ubuntu**: download **[Field.Notes_@VERSION@_amd64.deb](https://github.com/sparkly-quasar/field-notes/releases/download/@TAG@/Field.Notes_@VERSION@_amd64.deb)**, then
`sudo apt install ./Field.Notes_@VERSION@_amd64.deb`

**Fedora / RHEL**: download **[Field.Notes-@VERSION@-1.x86_64.rpm](https://github.com/sparkly-quasar/field-notes/releases/download/@TAG@/Field.Notes-@VERSION@-1.x86_64.rpm)**, then
`sudo dnf install ./Field.Notes-@VERSION@-1.x86_64.rpm`

### After installing

- **Saved your journal password in the macOS Keychain (Server Mode)?** Because
  the app isn't code-signed, macOS asks once after each update whether Field
  Notes may use it. Choose **Always Allow**. Until you do, the journal waits at
  the unlock screen.
- Your journal lives in your user app-data folder, separate from the app, so
  installing over an older version never touches it.

### The other files

`latest.json`, `Field.Notes_universal.app.tar.gz` and every `.sig` file are for
the in-app updater. You don't need to download them.
