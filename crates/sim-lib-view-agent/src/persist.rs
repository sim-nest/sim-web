//! Saving and loading a composed topology as SIM data.
//!
//! A topology saves and loads through the existing topology reflection -- there
//! is no second package format. `composer_save` reflects a `Graph` to a value;
//! `composer_load` parses a value back into a `Graph`.

use sim_kernel::{Cx, Expr, Result};
use sim_lib_topology::{Graph, graph_from_value, topology_reflect_graph};

/// Reflect a `Graph` to a SIM value for saving, sharing, or diffing.
pub fn composer_save(cx: &Cx, graph: &Graph) -> Expr {
    topology_reflect_graph(cx, graph)
}

/// Parse a saved SIM value back into a `Graph`.
pub fn composer_load(cx: &mut Cx, value: Expr) -> Result<Graph> {
    let value = cx.factory().expr(value)?;
    graph_from_value(cx, value)
}
