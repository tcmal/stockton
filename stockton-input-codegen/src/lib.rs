//! Macros for working with stockton_input

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident};

/// Generate an input manager for the given struct.
/// Each button in the struct should be decorated with #[button] and each axis with #[axis].
/// Given struct MovementInputs, this will output struct MovementInputsManager which implements InputManager.
/// It also creates an enum MovementInputsFields, with values for all the buttons and axes in MovementInputs.
/// You'll need to pass in an action schema to `MovementInputsManager::new()`, which is a BTreeMap<u32, (MovementInputsFields, InputMutation)>
/// You can then call `.handle_frame` on MovementInputsManager and then read the inputs from MovementInputsManager.inputs.
#[proc_macro_derive(InputManager, attributes(button, axis))]
pub fn derive_inputmanager(input: TokenStream) -> TokenStream {
    let struct_data = parse_macro_input!(input as DeriveInput);

    let visibility = &struct_data.vis;

    let struct_ident = &struct_data.ident;
    let manager_ident = format_ident!("{}Manager", struct_data.ident);
    let fields_enum_ident = format_ident!("{}Fields", struct_data.ident);

    let (buttons, axes) = get_categorised_idents(&struct_data.data);
    let caps_buttons = capitalise_idents(buttons.clone());
    let caps_axes = capitalise_idents(axes.clone());

    let fields_enum = gen_fields_enum(&fields_enum_ident, &caps_buttons, &caps_axes);
    let manager_struct = gen_manager_struct(
        &manager_ident,
        &struct_ident,
        &fields_enum_ident,
        buttons.len(),
    );
    let trait_impl = gen_trait_impl(
        &manager_ident,
        &struct_ident,
        &fields_enum_ident,
        &buttons,
        &axes,
        &caps_buttons,
        &caps_axes,
    );

    let expanded = quote! {
        #[derive(Debug, Clone, Copy)]
        #visibility #fields_enum

        #[derive(Debug, Clone)]
        #visibility #manager_struct

        #trait_impl

    };

    TokenStream::from(expanded)
}

/// Gets the buttons and axes from a given struct definition
/// Buttons are decorated with #[button] and axes with #[axis]
fn get_categorised_idents(data: &Data) -> (Vec<Ident>, Vec<Ident>) {
    let mut buttons = vec![];
    let mut axes = vec![];

    match data {
        Data::Struct(ref s) => match &s.fields {
            Fields::Named(fields) => {
                for field in fields.named.iter() {
                    let attrs = field.attrs.iter().map(|a| a.parse_meta().unwrap());
                    for attr in attrs {
                        if attr.path().is_ident("button") {
                            buttons.push(field.ident.as_ref().unwrap().clone());
                            break;
                        } else if attr.path().is_ident("axis") {
                            axes.push(field.ident.as_ref().unwrap().clone());
                            break;
                        }
                    }
                }
            }
            _ => unimplemented!(),
        },
        _ => {
            panic!("this is not a struct");
        }
    };

    (buttons, axes)
}

/// Convert a vector of idents to UpperCamel, as used in enums.
fn capitalise_idents(idents: Vec<Ident>) -> Vec<Ident> {
    idents
        .into_iter()
        .map(capitalise_ident)
        .collect::<Vec<Ident>>()
}

/// Convert a single ident to UpperCamel, as used in enums.
fn capitalise_ident(ident: Ident) -> Ident {
    format_ident!("{}", ident.to_string().to_case(Case::UpperCamel))
}

/// Generate an enum for the different buttons and axes in a struct.
///
/// Example output:
/// ```ignore
/// enum MovementInputsFields {
///     Jump,
///     Vertical,
///     Horizontal,
/// }
/// ```
fn gen_fields_enum(
    fields_enum_ident: &Ident,
    buttons_caps: &[Ident],
    axes_caps: &[Ident],
) -> TokenStream2 {
    quote!(
        enum #fields_enum_ident {
            #(#buttons_caps,)*
            #(#axes_caps,)*
        }
    )
}

/// Generates a manager struct for the given inputs struct with buttons_len buttons.
///
/// Example output:
/// ```ignore
/// struct MovementInputsManager {
///     inputs: MovementInputs,
///     actions: BTreeMap<Keycode, ActionResponse>,
///     just_hot: [bool; 1]
/// }
///
/// impl MovementInputsManager {
///     pub fn new(actions: BTreeMap<Keycode, ActionResponse>) -> Self {
///         MovementInputsManager {
///             inputs: MovementInputs {
///                 vertical: Axis::zero(),
///                 horizontal: Axis::zero(),
///                 jump: Button::new()
///             },
///             actions,
///             just_hot: [false]
///         }
///     }
/// }
/// ```
fn gen_manager_struct(
    ident: &Ident,
    struct_ident: &Ident,
    fields_enum_ident: &Ident,
    buttons_len: usize,
) -> TokenStream2 {
    let jh_falses = (0..buttons_len).map(|_| quote!(false));
    quote!(
        struct #ident {
            inputs: #struct_ident,
            actions: ::std::collections::BTreeMap<u32, (#fields_enum_ident, ::stockton_input::InputMutation)>,
            is_down: ::std::collections::BTreeMap<u32, bool>,
            just_hot: [bool; #buttons_len]
        }

        impl #ident {
            pub fn new(actions: ::std::collections::BTreeMap<u32, (#fields_enum_ident, ::stockton_input::InputMutation)>) -> Self {
                let mut is_down = ::std::collections::BTreeMap::new();
                for (k,_) in actions.iter() {
                    is_down.insert(*k, false);
                }

                #ident {
                    inputs: Default::default(),
                    actions,
                    is_down,
                    just_hot: [#(#jh_falses),*]
                }
            }
        }
    )
}

/// Implements the InputManager trait on a manager struct generated by gen_manager_struct.
///
/// Example output:
/// ```ignore
/// impl InputManager<Action> for MovementInputsManager {
///     fn handle_frame<X: IntoIterator<Item = Action>>(&mut self, actions: X) -> () {
///         // Set just hots back
///         if self.just_hot[0] {
///             self.inputs.jump.set_not_hot();
///             self.just_hot[0] = false;
///         }
///
///         // Deal with actions
///         for action in actions {
///             let mutation = self.actions.get(&action.keycode());
///
///             if let Some((field, mutation)) = mutation {
///                 let mut val = match mutation {
///                     InputMutation::MapToButton | InputMutation::PositiveAxis => 1,
///                     InputMutation::NegativeAxis => -1
///                 };
///                 if !action.is_down() {
///                     val *= -1
///                 }
///
///                 match field {
///                     MovementInputsFields::Jump => {
///                         self.inputs.jump.modify_inputs(val > 0);
///                         self.just_hot[0] = true;
///                     },
///                     MovementInputsFields::Vertical => {
///                         self.inputs.vertical.modify(val);
///                     },
///                     MovementInputsFields::Horizontal => {
///                         self.inputs.horizontal.modify(val);
///                     }
///                 }
///             }
///         }
///     }
/// }
/// ```
fn gen_trait_impl(
    manager: &Ident,
    struct_ident: &Ident,
    fields_enum: &Ident,
    buttons: &[Ident],
    axes: &[Ident],
    buttons_caps: &[Ident],
    axes_caps: &[Ident],
) -> TokenStream2 {
    let just_hot_resets = gen_just_hot_resets(&buttons);
    let field_match_modify =
        gen_field_mutation(&buttons, &axes, &buttons_caps, &axes_caps, &fields_enum);

    quote!(
        impl InputManager for #manager {
            type Inputs = #struct_ident;

            fn handle_frame<'a, X: IntoIterator<Item = &'a ::stockton_input::Action>>(&mut self, actions: X) -> () {
                #(#just_hot_resets)*

                for action in actions {
                    let mutation = self.actions.get(&action.keycode());

                    if let Some((field, mutation)) = mutation {
                        if *self.is_down.get(&action.keycode()).unwrap() == action.is_down() {
                            // Duplicate event
                            continue;
                        }

                        self.is_down.insert(action.keycode(), action.is_down());

                        use ::stockton_input::InputMutation;

                        let mut val = match mutation {
                            InputMutation::MapToButton | InputMutation::PositiveAxis => 1,
                            InputMutation::NegativeAxis => -1
                        };
                        if !action.is_down() {
                            val *= -1
                        }

                        #field_match_modify
                    }
                }
            }

            fn get_inputs(&self) -> &Self::Inputs {
                &self.inputs
            }
        }
    )
}

/// Generate the if statements used to reset self.just_hot at the start of each frame
/// Used by gen_trait_impl.
fn gen_just_hot_resets(buttons: &[Ident]) -> Vec<TokenStream2> {
    buttons
        .iter()
        .enumerate()
        .map(|(i, v)| {
            quote!(
                if self.just_hot[#i] {
                    self.inputs.#v.set_not_hot();
                    self.just_hot[#i] = false;
                }
            )
        })
        .collect()
}

/// Generate the code that actually mutates an input field by matching on a fields enum.
/// Used by gen_trait_impl.
fn gen_field_mutation(
    buttons: &[Ident],
    axes: &[Ident],
    buttons_caps: &[Ident],
    axes_caps: &[Ident],
    fields_enum_ident: &Ident,
) -> TokenStream2 {
    let arms = {
        let mut btn_arms: Vec<TokenStream2> =
            gen_mutate_match_arms_buttons(buttons, &buttons_caps, fields_enum_ident);
        let mut axes_arms = gen_mutate_match_arms_axes(axes, &axes_caps, fields_enum_ident);

        btn_arms.append(&mut axes_arms);

        btn_arms
    };

    quote!(
        match field {
            #(#arms),*
        };
    )
}

/// Used by gen_field_mutation.
fn gen_mutate_match_arms_buttons(
    buttons: &[Ident],
    buttons_caps: &[Ident],
    fields_enum_ident: &Ident,
) -> Vec<TokenStream2> {
    buttons
        .iter()
        .enumerate()
        .zip(buttons_caps.iter())
        .map(|((idx, field), cap)| {
            quote!(
                #fields_enum_ident::#cap => {
                    self.inputs.#field.modify_inputs(val > 0);
                    self.just_hot[#idx] = true;
                }
            )
        })
        .collect::<Vec<TokenStream2>>()
}

/// Used by gen_field_mutation.
fn gen_mutate_match_arms_axes(
    axes: &[Ident],
    axes_caps: &[Ident],
    fields_enum_ident: &Ident,
) -> Vec<TokenStream2> {
    axes.iter()
        .zip(axes_caps.iter())
        .map(|(field, cap)| {
            quote!(
                #fields_enum_ident::#cap => {
                    self.inputs.#field.modify(val);
                }
            )
        })
        .collect::<Vec<TokenStream2>>()
}
