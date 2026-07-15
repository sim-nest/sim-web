# sim-lib-view-bridge

In one line: it lets people review and change BRIDGE packets in the same form agents use.

## What it gives you

Open a BRIDGE packet and see its sender, move, profile, and parts in a surface-friendly review pane. A person can propose a patch, write a review, cast a scored vote, or record a receipt without leaving the packet model. The edit becomes the same collaboration record an agent reads, so there is one shared object instead of one browser copy and one model copy.

## Why you will be glad

- Reviews and patches stay attached to the packet they discuss.
- Human and agent edits compare directly because they decode to the same records.
- Receipts make accepted changes visible as data, not sidebar notes.

## Where it fits

This crate sits between the BRIDGE packet codec and the SIM Web surface stack. It gives the workspace a packet review seat while keeping display and edit behavior inside the existing scene, intent, and surface contracts.
