# Actio

Rust framework for long‑running async actions that produces a stream of feedback, supports cancellation, and eventually resolves to a terminal result.
Feedback and cancellation is selected at compile-time, therefore you do not pay for the features you do not opt-in.

### Features

- **Feedback Streams** - Progress updates 
- **Cancellation** - Graceful task cancellation with (optional) context
- **State Snapshots** - Capture task state at cancellation time
- **Stateful servers** - Allows to visit the server and update its internal state based on `Outcome`


### Comparison with ROS actions

`actio` aims to provide functionality similar to ROS actions, but with the following design differences:

- Used within the same process. Unlike ROS that uses its middleware to manage the multi-nodal execution `actio` is limited to use cases where the client and the server lives in the same process space.
This avoids the de/serialization and communication overhead and allows for cleaner design i.e. using `TaskHandle` that correspond to each `Goal` instead of relying on `UUID` to distinguish goals.
- `actio` uses event-based instead of polling-based cancellation mechanism. This allows clients to have more control over task execution. In practice this leads to cleaner server logic implementation - cancellation acceptance and handling does not need to be checked several times. To cleanup or update server state on cancellation `actio` provides a mechanism to visit the state.
Its important to keep in mind that tasks defined on the server side must be cancel-safe.
- Strong types and optional features. Feedback and cancellation is purely optional, if you opt-out you do not pay any performance overhead.


### Design Diagram
![Design Diagram](docs/design.excalidraw.png)
