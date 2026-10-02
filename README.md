# wholocks

`wholocks` finds Windows processes that have an open handle to a file or folder.
It is useful when File Explorer says that a folder or a file is in use and cannot
be deleted or moved.

## Run from source

From this project folder, scan a file or folder:

```powershell
cargo run -- "C:\Users\HExCR\Documents\Prog\Svelte\auth-system"
```

The program prints the executable path, PID, and the exact handle path for each
matching process.

## Build the release executables

```powershell
cargo build --release --bins
.\target\release\wholocks.exe "C:\Users\HExCR\Documents\Prog\Svelte\auth-system"
```

The build creates two programs:

- `wholocks.exe` is the command-line tool.
- `wholocks-gui.exe` is the GPUI window used by File Explorer.

## Add WhoLocks to File Explorer

Build the release executables, then register the context-menu command for your
Windows user:

```powershell
.\scripts\install-shell-verb.ps1
```

This creates
`HKCU\Software\Classes\AllFilesystemObjects\shell\WhoLocks` and directly runs
`wholocks-gui.exe "%1"`. Administrator privileges are not required.

Right-click one file or folder and choose **Show more options > WhoLocks**.
The window shows one row per locking PID, with `End Task` on each row and
`End All Tasks` in the top-right. Termination always asks for confirmation.

The registration points to the current release directory. Do not move or delete
the two executables after installing the menu. To remove the menu:

```powershell
.\scripts\uninstall-shell-verb.ps1
```

The equivalent built-in commands are `wholocks.exe --install-context-menu` and
`wholocks.exe --uninstall-context-menu`.

You can also open the UI directly:

```powershell
.\target\release\wholocks-gui.exe "C:\path\to\some-file.txt"
```

To check one file instead of an entire folder:

```powershell
.\target\release\wholocks.exe "C:\path\to\some-file.txt"
```

## Terminate locking processes

First scan the path and inspect the output. To force-close every distinct process
found holding that path, use `--kill`:

```powershell
cargo run -- --kill "C:\Users\HExCR\Documents\Prog\Svelte\auth-system"
```

Or with the release executable:

```powershell
.\target\release\wholocks.exe --kill "C:\Users\HExCR\Documents\Prog\Svelte\auth-system"
```

`--kill` terminates processes immediately. They do not get a chance to save
unsaved work or clean up. It may close your editor, terminal, development server,
or File Explorer, so use it only after checking the scan output.

## Access limitations

Windows can hide handles belonging to protected processes. If a scan shows no
matching process while Windows still says the item is in use, open PowerShell as
Administrator and run the same command again.
