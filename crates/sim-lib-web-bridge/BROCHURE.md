# sim-lib-web-bridge

In one line: it is the pipe that carries your edits out and the fresh pictures back, no matter where the runtime actually runs.

## What it gives you

The bridge connects the browser workspace to a running system without you caring where that system lives. It can talk to a runtime inside the same browser tab, a server on your own machine, or a server across the network, and it uses one common way of asking rather than a different setup for each. For tests, it can replay a recorded session so results come out the same every time. Both a person at the browser and an automated agent are equal peers on this one channel, so edits and updated screens flow through the same place for everyone.

## Why you will be glad

- The workspace works the same whether the runtime is local or remote.
- Recorded sessions let tests replay real traffic and get identical results.
- People and agents share one channel, so no one is a second-class participant.

## Where it fits

This is the transport layer of the SIM browser workspace. It carries edit requests from the view stack to a runtime and streams scene differences back to the panes that need them. By targeting SIM's location-independent evaluation surface instead of any one network path, it lets the whole workspace stay the same while the runtime behind it moves.
