use quote::quote;
use syn::{braced, parse::Parse, parse_macro_input, punctuated::Punctuated, Ident, Token};

type StateId = Ident;
type ActionId = Ident;

struct StateTransitions {
    state: StateId,
    transitions: Vec<Transition>,
}

impl StateTransitions {
    fn check_transitions_consistency(&self) -> bool {
        // TODO: check which ones are conflicting exactly...
        let number_actions = self
            .transitions
            .iter()
            .map(|t| t.actions.len())
            .sum::<usize>();
        let number_unique_actions = self
            .transitions
            .iter()
            .flat_map(|t| t.actions.iter())
            .collect::<std::collections::HashSet<_>>()
            .len();
        number_actions == number_unique_actions
    }
}

impl Parse for StateTransitions {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let state = input.parse::<Ident>()?;
        let transitions_def;
        braced!(transitions_def in input);
        let transitions = Punctuated::<Transition, Token![,]>::parse_terminated(&transitions_def)?
            .into_iter()
            .collect();
        Ok(StateTransitions { state, transitions })
    }
}

struct Transition {
    actions: Vec<ActionId>,
    next_states: Vec<StateId>,
}

impl Parse for Transition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let actions = Punctuated::<Ident, Token![|]>::parse_separated_nonempty(input)?
            .into_iter()
            .collect();
        input.parse::<Token![=>]>()?;
        let next_states = Punctuated::<Ident, Token![|]>::parse_separated_nonempty(input)?
            .into_iter()
            .collect();
        Ok(Transition {
            actions,
            next_states,
        })
    }
}

struct StateMachineDefinition {
    state_wrapper: Ident,
    action_wrapper: Ident,
    state_transitions: Vec<StateTransitions>,
}

impl Parse for StateMachineDefinition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let state_wrapper = input.parse::<Ident>()?;
        input.parse::<Token![,]>()?;
        let action_wrapper = input.parse::<Ident>()?;
        input.parse::<Token![,]>()?;
        let state_transitions = Punctuated::<StateTransitions, Token![,]>::parse_terminated(input)?
            .into_iter()
            .collect();

        Ok(StateMachineDefinition {
            state_wrapper,
            action_wrapper,
            state_transitions,
        })
    }
}

fn define_wrappers(smd: &StateMachineDefinition) -> proc_macro2::TokenStream {
    let state_wrapper = &smd.state_wrapper;
    let action_wrapper = &smd.action_wrapper;

    let mut states = std::collections::HashSet::new();
    let mut actions = std::collections::HashSet::new();
    for st in &smd.state_transitions {
        states.insert(&st.state);
        for t in &st.transitions {
            for a in &t.actions {
                actions.insert(a);
            }

            for next_s in &t.next_states {
                states.insert(next_s);
            }
        }
    }

    let mut state_from_impl_acc = quote! {};
    let mut state_acc = quote! {};
    for s in states {
        state_acc = quote! {
            #state_acc
            #s(#s),
        };

        state_from_impl_acc = quote! {
            #state_from_impl_acc

            impl From<#s> for #state_wrapper {
                fn from(s: #s) -> #state_wrapper {
                    #state_wrapper::#s(s)
                }
            }
        };
    }

    let mut action_acc = quote! {};
    let mut action_trait_impl_acc = quote! {};
    let mut action_from_impl_acc = quote! {};
    for a in actions {
        action_acc = quote! {
            #action_acc
            #a(#a),
        };

        action_trait_impl_acc = quote! {
            #action_trait_impl_acc
            impl Action for #a {}
        };

        action_from_impl_acc = quote! {
            #action_from_impl_acc

            impl From<#a> for #action_wrapper {
                fn from(a: #a) -> #action_wrapper {
                    #action_wrapper::#a(a)
                }
            }
        };
    }

    quote! {
        #[derive(Debug)]
        enum #state_wrapper {
            #state_acc
        }

        #state_from_impl_acc

        #[derive(Debug)]
        enum #action_wrapper {
            #action_acc
        }

        #action_trait_impl_acc
        #action_from_impl_acc
    }
}

fn define_transition(st: &StateTransitions, action_wrapper: &Ident) -> proc_macro2::TokenStream {
    let start_state = &st.state;

    let mut action_to_lambda_acc = quote! {};
    for t in &st.transitions {
        if t.next_states.is_empty() {
            continue;
        }

        let mut assert_acc = quote! {};
        for output_state in &t.next_states {
            assert_acc = quote! {
                #assert_acc
                matches!(n, &Self::#output_state(_)) ||
            };
        }

        let assert_as_str = assert_acc.to_string();
        for a in &t.actions {
            let state_as_str = start_state.to_string();
            let action_as_str = a.to_string();
            action_to_lambda_acc = quote! {
                #action_to_lambda_acc
                #action_wrapper::#a(_) => |n| if !(#assert_acc false) { panic!("For state {:#?} and action {:#?}, got wrong state: {:#?}, matched against: {:#?}", #state_as_str, #action_as_str, n, #assert_as_str); },
            };
        }
    }

    let mut action_dispatch = quote! {};
    for t in &st.transitions {
        for a in &t.actions {
            action_dispatch = quote! {
                #action_dispatch
                #action_wrapper::#a(a) => state.next(a),
            };
        }
    }

    quote! {
        Self::#start_state(state) => {
            let assert_lambda =
            match &action {
                #action_to_lambda_acc
                _ => |_| (),
            };

            let next_state = match action {
                #action_dispatch
                _ => return Err((Self::#start_state(state), action)),
            };
            assert_lambda(&next_state);
            next_state
        }
    }
}

fn define_loop(smd: &StateMachineDefinition) -> proc_macro2::TokenStream {
    let state_wrapper = &smd.state_wrapper;
    let action_wrapper = &smd.action_wrapper;

    let mut acc = quote! {};

    for st in &smd.state_transitions {
        let transition_case = define_transition(st, action_wrapper);
        acc = quote! {
            #acc
            #transition_case
        };
    }

    quote! {
        impl #state_wrapper {
            fn next(self, action: #action_wrapper) -> Result<#state_wrapper, (#state_wrapper, #action_wrapper)> {
                Ok(match self  {
                    #acc
                    terminal_state => terminal_state
                })
            }
        }
    }
}

#[proc_macro]
pub fn state_machine(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let smd = parse_macro_input!(item as StateMachineDefinition);
    for st in &smd.state_transitions {
        if !st.check_transitions_consistency() {
            panic!(
                "Some pair (state, action) have been defined multiple times for state {}",
                st.state
            );
        }
    }

    let wrappers = define_wrappers(&smd);
    let fsm_impl = define_loop(&smd);

    quote! {
        #wrappers
        #fsm_impl

    }
    .into()
}
