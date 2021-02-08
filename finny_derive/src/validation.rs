use std::collections::{HashMap, HashSet};

use petgraph::{Graph, graph::NodeIndex, visit::Dfs};
use proc_macro2::Span;
use syn::spanned::Spanned;

use crate::{parse::{FsmDeclarations, FsmRegion, ValidatedFsm}, parse_fsm::FsmCodegenOptions, utils::tokens_to_string};

#[derive(Debug)]
struct TypeNode {
    state: syn::Type,
    region: Option<usize>
}

pub fn create_regions(decl: FsmDeclarations, options: FsmCodegenOptions) -> syn::Result<ValidatedFsm> {
    let mut graph = Graph::new();
    let mut nodes = HashMap::new();

    fn get_or_add_node(nodes: &mut HashMap<syn::Type, NodeIndex>, graph: &mut Graph::<TypeNode, i32>, ty: &syn::Type) -> NodeIndex {
        *nodes.entry(ty.clone())
            .or_insert_with(|| {
                let node = TypeNode { state: ty.clone(), region: None };
                graph.add_node(node)
            })
    }

    for (ty, _) in &decl.states {
        get_or_add_node(&mut nodes, &mut graph, ty);
    }

    for transition in &decl.transitions {
        let states = transition.ty.get_states();
        for state in &states {
            get_or_add_node(&mut nodes, &mut graph, state);
        }
        match states.as_slice() {
            [from, to] => {
                let state_from = get_or_add_node(&mut nodes, &mut graph, from);
                let state_to = get_or_add_node(&mut nodes, &mut graph, to);

                graph.add_edge(state_from, state_to, 0);
            },
            _ => ()
        }
    }

    for (region_id, initial_state) in decl.initial_states.iter().enumerate() {
        let start_node = get_or_add_node(&mut nodes, &mut graph, initial_state);
        let mut dfs = Dfs::new(&graph, start_node);
        while let Some(idx) = dfs.next(&graph) {
            if idx != start_node && graph[idx].region.is_some() {
                let s = &graph[idx].state;
                return Err(syn::Error::new(s.span(), &format!("The state '{}' was already matched into another region, check the transition graph of the states!",
                tokens_to_string(s))));
            }
            graph[idx].region = Some(region_id);
        }
    }

    for node in graph.raw_nodes() {
        if node.weight.region == None {
            return Err(syn::Error::new(node.weight.state.span(), "Unreachable state! Add some transitions that will make this state reachable!"));
        }
    }

    // build the regions
    let mut regions = vec![];
    for (region_id, initial_state) in decl.initial_states.iter().enumerate() {
        let (transitions, states) = {
            let region_states: HashSet<_> = graph.raw_nodes().iter()
                .filter(|n| n.weight.region == Some(region_id))
                .map(|n| n.weight.state.clone())
                .collect();

            let mut transitions = vec![];
            for transition in &decl.transitions {
                let states = transition.ty.get_states();
                if states.len() == 0 {
                    return Err(syn::Error::new(Span::call_site(), "No states for this transition found, codegen bug!"));
                }

                let c = states.iter().filter(|s| region_states.contains(s)).count();
                
                if c == states.len() {
                    transitions.push(transition.clone());
                } else if c != 0 {
                    return Err(syn::Error::new(Span::call_site(), "Only some states belong to this region, codegen bug!"));
                }
            }

            (transitions, region_states)
        };

        regions.push(FsmRegion {
            initial_state: initial_state.clone(),
            region_id,
            transitions,
            states: states.into_iter().map(|ty| decl.states.get(&ty).unwrap()).cloned().collect()
        });
    }
    
    Ok(ValidatedFsm {
        events: decl.events,
        states: decl.states,
        regions,
        codegen_options: options
    })
}