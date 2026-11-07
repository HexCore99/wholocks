# Changelog

## 0.9.2

- Added the purple file-and-keyhole logo as the Windows application icon.
- Added icon sizes from 16 to 256 pixels for the executables, app window, taskbar, and Explorer integration.
- Updated the Windows installer to use the same logo.

## 0.9.1

- Optimized release builds with thin link-time optimization and a single code-generation unit.
- Removed debug information from release executables.

## 0.9.0

- Added a GPUI window with one row per locking process.
- Added per-process **End Task** and bulk **End All Tasks** actions with confirmation.
- Added administrator retry when Windows blocks process inspection.
- Added a standard Windows shell verb for files and folders.
- Added a per-user Windows installer and uninstaller.
