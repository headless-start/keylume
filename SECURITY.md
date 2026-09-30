# Security

Keylume runs on your PC, talks to your keyboard over USB and reads files you pick. It opens
no network connections at all. If you find a way
to make it do something it shouldn't, please tell us privately first.

## Reporting a vulnerability

Use GitHub's private reporting: on the repository page, open **Security → Report a
vulnerability**. Please include:

- what an attacker needs first (for example: a file you'd have to open, a program already
  running on your PC, or a web page open in your browser);
- the steps, and what happens;
- the Keylume version (Settings shows it) and your operating system.

Please don't open a public issue for it. We'll answer as soon as we can, fix what's confirmed,
and credit you in the changelog unless you'd rather not be named.

## Supported versions

Only the newest release gets security fixes.

## What's in scope

- The app's own commands (the window can only ask for what they allow), file handling
  (profiles, backups, settings, packs).
- Anything that writes to the keyboard: Keylume checks a device is a supported keyboard before
  writing to it, and checks backups in full before restoring them. A way around either is a
  vulnerability.
- The installer and release artifacts.

Out of scope: problems that need an attacker who can already run programs as you (they can do
anything Keylume can), and the keyboard's own firmware (report those to its maker).
