# Session Manager

Session Manager is a work context management tool for Linux.

It groups workspaces, windows, applications, and processes into logical sessions, allowing users to switch contexts without mixing ongoing activities. A session does not fully isolate programs. Instead, it determines what is visible, accessible, and relevant within the active context.

The project aims to:

- associate workspaces and windows with a session;
- filter navigation and displayed elements based on the active session;
- adapt the applications offered by the launcher;
- preserve or restore useful session state;
- manage applications, profiles, and processes specific to each context;
- expose an interface for the desktop shell and other tools.

Initial development targets an environment based on Hyprland and Quickshell. This repository contains the source code, functional and technical documentation, design decisions, and implementation tracking.

## Project documentation

- [`ToDo.md`](ToDo.md) tracks planned and completed work.
- [`AGENTS.md`](AGENTS.md) contains working instructions for AI agents.
- [`docs/documentation-structure.md`](docs/documentation-structure.md) defines the documentation structure and sources of truth.

## Project status

The project is currently in the design phase. The expected behavior, architecture, and scope of the first version still need to be defined before implementation begins.
