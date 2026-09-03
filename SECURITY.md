# Security

## Reporting a vulnerability

Use GitHub's private vulnerability reporting on this repository
(Security tab, "Report a vulnerability"). Do not open a public issue for
anything that could put users at risk before a fix is out.

A report is acknowledged within a week. Fixes ship as a patch release
and are noted in the release notes once users have had a chance to
update.

## What counts

riso writes into the user's configuration directory and runs the reload
commands of the desktop it detects. Anything that lets a theme, a plugin
or a catalog entry do more than that is in scope: writing outside the
state and config trees, running code that a theme carried, escaping the
ownership store so `riso restore` cannot undo a change.

Themes are data by contract. `riso theme validate` and the install gate
exist to enforce that; a way around them is a vulnerability.

## Supported versions

The latest release on the `main` branch. Older releases get no
backports.
