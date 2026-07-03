# sim-lib-view

In one line: it decides how any value should be shown and edited, and always has a sensible default ready.

## What it gives you

A lens is a matched pair: a way to display a value and a way to edit it back. This crate chooses the right lens for whatever you open, using the same matching the runtime already uses elsewhere, so there is no second set of rules to learn. If nothing special fits, a universal default lens still shows the value cleanly, at a depth that suits you -- a light household view, a builder view, or a full systems view. You can stack lenses to look at one value several ways at once. Opening something unfamiliar never leaves you staring at a blank pane.

## Why you will be glad

- Anything you open shows up in a usable form, even with no custom view for it.
- You can dial the detail up or down to match how closely you want to look.
- Editing is built in beside viewing, so what you see is what you can change.

## Where it fits

This is the heart of the SIM view stack. Every specialized lens family -- math, audio, documents, agent graphs, codecs -- plugs into the selection and stacking rules defined here. It ties the display side to the editing side and guarantees a fallback, so the workspace stays useful across every kind of value the runtime can hold.
