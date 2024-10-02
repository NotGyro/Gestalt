#![feature(string_remove_matches)]

use std::collections::HashMap;

use proc_macro::TokenStream;
use proc_macro2::TokenTree;
use quote::{quote, ToTokens, format_ident};
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, DeriveInput, Field, Ident, LitInt, Type, Token};
extern crate proc_macro2;

struct NetMsgAttr {
	id_lit: Option<LitInt>,
	id_ident: Option<Ident>,
	sidedness: Ident,
	guarantee: Ident,
	stream_select: Option<Ident>,
}

impl Parse for NetMsgAttr {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut id_lit = None;
		let mut id_ident = None;
		if input.peek(LitInt) {
			id_lit = input.parse()?;
		} else if input.peek(Ident) {
			id_ident = input.parse()?;
		} else {
			return Err(input.error("Expected literal integer or identifier"));
		}
		input.parse::<Token![,]>()?;
		let sidedness = input.parse()?;
		input.parse::<Token![,]>()?;
		let guarantee = input.parse()?;
		let stream_select = if !input.is_empty() {
			input.parse::<Token![,]>()?;
			Some(input.parse()?)
		} else {
			None
		};
		Ok(NetMsgAttr {
			id_lit,
			id_ident,
			sidedness,
			guarantee,
			stream_select,
		})
	}
}

#[proc_macro_attribute]
pub fn netmsg(attr: TokenStream, item: TokenStream) -> TokenStream {
	let NetMsgAttr {
		id_lit,
		id_ident,
		sidedness,
		guarantee,
		stream_select,
	} = parse_macro_input!(attr as NetMsgAttr);
	let stream_select = match stream_select {
		Some(s) => quote! { crate::net::netmsg::StreamSelector::Specific(#s) },
		None => quote! { crate::net::netmsg::StreamSelector::Any },
	};

	let id = if let Some(i) = id_lit {
		quote!(#i)
	} else if let Some(i) = id_ident {
		quote!(#i)
	} else {
		unreachable!()
	};

	let tokens = item.clone();
	let msg_struct = parse_macro_input!(tokens as syn::ItemStruct);
	let message = msg_struct.ident;

	let item: syn::Item = syn::parse(item).expect("failed to parse item");

	(quote! {
	#item

	impl crate::net::NetMsg for #message {
		#[inline(always)]
		fn net_msg_id() -> u32 { #id }
		#[inline(always)]
		fn net_msg_guarantees() -> crate::net::netmsg::PacketGuarantees { crate::net::netmsg::PacketGuarantees::#guarantee }
		#[inline(always)]
		fn net_msg_stream() -> crate::net::netmsg::StreamSelector { #stream_select }
		#[inline(always)]
		fn net_msg_name() -> &'static str { stringify!(#message) }
		#[inline(always)]
		fn net_msg_sidedness() -> crate::net::netmsg::MessageSidedness {
			crate::net::netmsg::MessageSidedness::#sidedness
		}
	}

	impl TryInto<crate::net::netmsg::PacketIntermediary> for &#message {
		type Error = Box<dyn std::error::Error>;
		fn try_into(self) -> Result<crate::net::netmsg::PacketIntermediary, Box<dyn std::error::Error>> {
			use crate::net::netmsg::NetMsg;
			self.construct_packet()
		}
	}
		})
	.into()
}

#[proc_macro_derive(ChannelSet, attributes(channel, domain_channel))]
pub fn impl_channel_set(channel_set: TokenStream) -> TokenStream {
	const CHANNEL_STR: &'static str = "channel";
	const DC_STR: &'static str = "domain_channel";
	const DOMAIN_SUFFIX: &'static str = "_domain";
	const STATIC_BUILDER_SUFFIX: &'static str = "Fields";

	pub enum SubsetKind { 
		Channel,
		Sender,
		Receiver,
	}
	impl SubsetKind { 
		fn from_ty(ty: &Type) -> Self { 
			let token_str = ty.to_token_stream()
				.to_string()
				.to_lowercase();
			if token_str.contains("receiver") { 
				Self::Receiver
			}
			else if token_str.contains("sender") {
				Self::Sender
			}
			else {
				Self::Channel
			}
		}
	}
	struct IdentifiedChannel {
		pub field_name: Ident,
		pub static_channel: Ident,
		pub ty: syn::Type,
		pub subset_kind: SubsetKind,
	}
	impl IdentifiedChannel { 
		fn from_field(field: &Field, attr_tokens: &Vec<TokenTree>) -> Self {
			let field_ident = field.ident.clone().unwrap();
			let token_tree = attr_tokens.first().unwrap();
			let subset_kind = SubsetKind::from_ty(&field.ty);
			if let TokenTree::Ident(channel_ident) = token_tree { 
				return IdentifiedChannel {
					field_name: field_ident,
					static_channel: channel_ident.clone(),
					ty: field.ty.clone(),
        			subset_kind,
				};
			}
			else { 
				panic!("Non-ident for channel field!");
			}
		}
	}
	struct IdentifiedDomainChannel { 
		pub inner: IdentifiedChannel,
		pub domain_suffixed: Ident,
	}
	impl IdentifiedDomainChannel { 
		fn from_field(field: &Field, attr_tokens: &Vec<TokenTree>) -> Self {
			let inner = IdentifiedChannel::from_field(field, attr_tokens);
			let domain_lit = attr_tokens[1..].iter().find_map(|tree| {
				if let TokenTree::Literal(val) = tree { 
					return Some(val);
				}
				None
			}).expect("No domain identifier provided for domain channel!");
			let mut domain_ident = domain_lit.to_string();
			domain_ident.remove_matches("\"");
			domain_ident.remove_matches("\'");
			let domain_suffixed = format_ident!("{domain_ident}{DOMAIN_SUFFIX}");
			Self {
				inner,
				domain_suffixed,
			}
		}
	}
	let parsed = parse_macro_input!(channel_set as DeriveInput);
	
	let struct_ident = parsed.ident.clone();
	if let syn::Data::Struct(struct_data) = parsed.data {
		let our_attributes = vec![CHANNEL_STR,
			DC_STR,];

		let mut identified_channels: Vec<IdentifiedChannel> = Vec::new();
		let mut identified_domain_channels: Vec<IdentifiedDomainChannel> = Vec::new();
		let mut domains_for_builder = HashMap::new();
		let mut domains_for_channels = HashMap::new();
		let mut non_channel_fields: Vec<Field> = Vec::new();
		// Find fields with #[channel(T)] attributes 
		for field in struct_data.fields.iter() {
			let mut non_channel = true;
			for attr in field.attrs.iter() {
				let meta = attr.meta.require_list().unwrap();
				// Check to see if this is *our* attribute and not something else.
				let attribute_parsed = meta.path.segments.first().unwrap().ident.to_string();
				if meta.path.segments.len() == 1 &&
					our_attributes.contains(&attribute_parsed.as_str()) {
					non_channel = false; 
					// Extract channel type name from attr
					let attr_tokens: Vec<TokenTree> = (&meta.tokens).clone().into_iter().collect();
					// Should only ever register one field to one channel identifier
					match attribute_parsed.as_str() { 
						CHANNEL_STR => {
							assert!(attr_tokens.len() == 1);
							// Extract channel type name from attr
							let attr_tokens: Vec<TokenTree> = (&meta.tokens).clone().into_iter().collect();
							// Should only ever register one field to one channel identifier
							assert!(attr_tokens.len() == 1);
							let channel = IdentifiedChannel::from_field(field, &attr_tokens);
							identified_channels.push(channel);
						},
						DC_STR => {
							let attr_tokens: Vec<TokenTree> = (&meta.tokens).clone().into_iter().collect();
							let channel: IdentifiedDomainChannel = IdentifiedDomainChannel::from_field(field, &attr_tokens);
							domains_for_builder.insert(channel.domain_suffixed.clone(), channel.inner.static_channel.clone());
							domains_for_channels.insert(channel.inner.static_channel.clone(), channel.domain_suffixed.clone());
							identified_domain_channels.push(channel);
						},
						_ => {}, //Not one of ours, ignore.
					}
				}
			}
			//None of our attributes? Do this instead.
			if non_channel {
				non_channel_fields.push(field.clone())
			}
		}
		// We know which fields are identified channels now. 
		// Iterate through and impl HasChannel<>
		let mut impls: proc_macro2::TokenStream = proc_macro2::TokenStream::new();
		// Field lines for from_subset()
		let mut fields_clone = proc_macro2::TokenStream::new();
		// Also do our ridiculous where clause.
		let mut where_args = proc_macro2::TokenStream::new();
		// For subset builder stuff such as domains.
		let mut subset_builder_fields = proc_macro2::TokenStream::new();
		// Loop through, appending each HasChannel impl to our implementations.
		for IdentifiedChannel{field_name, static_channel, ty, subset_kind} in identified_channels.iter() {
			match subset_kind {
				SubsetKind::Channel => {
					impls.extend(quote!{
						impl crate::common::message::HasChannel<#static_channel> for #struct_ident { 
							fn get_channel(&self) -> &#ty { 
								&self.#field_name
							}
						}
					});
					where_args.extend(quote!{T: crate::common::message::HasChannel<#static_channel>,});
					fields_clone.extend(quote!{#field_name: <T as crate::common::message::HasChannel<#static_channel>>::get_channel(parent).clone().into(), });
				},
				SubsetKind::Sender => {
					impls.extend(quote!{
						impl crate::common::message::HasSender<#static_channel> for #struct_ident { 
							fn get_sender(&self) -> &#ty { 
								&self.#field_name
							}
						}
					});
					where_args.extend(quote!{T: crate::common::message::StaticSenderSubscribe<#static_channel>,});
					fields_clone.extend(quote!{#field_name: <T as crate::common::message::StaticSenderSubscribe<#static_channel>>::sender_subscribe(parent).into(), });

				}
				SubsetKind::Receiver => {
					impls.extend(quote!{
						impl crate::common::message::HasReceiver<#static_channel> for #struct_ident { 
							fn get_receiver(&self) -> &#ty { 
								&self.#field_name
							}
						}
					});
					where_args.extend(quote!{T: crate::common::message::StaticReceiverSubscribe<#static_channel>,});
					fields_clone.extend(quote!{#field_name: <T as crate::common::message::StaticReceiverSubscribe<#static_channel>>::receiver_subscribe(parent).into(), });
				}
			}
		}
		let mut use_non_channel_fields = false; 
		for field in non_channel_fields {
			let type_string = field.ty.to_token_stream().to_string();
			// Make sure we don't force people to muck around with PhantomData in builders.
			if !type_string.contains("PhantomData") {
				use_non_channel_fields = true;
				subset_builder_fields.extend(quote!{#field});
			}
			else {
				let field_ident = field.ident.unwrap().clone();
				fields_clone.extend(quote!{#field_ident: std::marker::PhantomData});
			}
		}
		if identified_domain_channels.is_empty() && !use_non_channel_fields { 
			// Now actually implement our from_subset() behavior
			// Fields_clone was built ahead-of-time because it is *irritating*
			// to concatenate tokens inside another token stream. However, 
			// it would make more sense - for comprehending this code, assume #fields_clone
			// is being built inside the quote block here somehow. That's the only place it gets used.
			impls.extend(quote!{
				impl crate::common::message::ChannelSet for #struct_ident { 
					type StaticBuilder = ();
				}
				impl<T> crate::common::message::CloneSubset<T> for #struct_ident where #where_args { 
					fn from_subset(parent: &T) -> Self {
						#struct_ident {
							#fields_clone
						}
					}
				}
			});
		}
		else {
			// We have to jump through some hoops here, in this case. 
			for (domain, static_channel) in domains_for_builder {
				subset_builder_fields.extend(quote!{#domain: <#static_channel as crate::common::message::StaticDomainChannelAtom>::Domain,});
			}
			let builder_ident = format_ident!("{struct_ident}{STATIC_BUILDER_SUFFIX}");
			impls.extend(quote!{
				pub struct #builder_ident {
					#subset_builder_fields
				}
				impl crate::common::message::ChannelSet for #struct_ident { 
					type StaticBuilder = #builder_ident;
				}
			});
			for IdentifiedDomainChannel{inner: IdentifiedChannel{field_name, static_channel, ty, subset_kind}, domain_suffixed} in identified_domain_channels.iter() {
				match subset_kind {
					SubsetKind::Channel => {
						// This is the easy(ish) one
						impls.extend(quote!{
							impl crate::common::message::HasChannel<#static_channel> for #struct_ident { 
								fn get_channel(&self) -> &#ty { 
									&self.#field_name
								}
							}
						});
						where_args.extend(quote!{T: crate::common::message::HasChannel<#static_channel>,});
						fields_clone.extend(quote!{#field_name: <T as crate::common::message::HasChannel<#static_channel>>::get_channel(parent).clone().into(), });
					},
					SubsetKind::Sender => {
						impls.extend(quote!{
							impl crate::common::message::HasSender<#static_channel> for #struct_ident { 
								fn get_sender(&self) -> &#ty {
									&self.#field_name
								}
							}
						});
						where_args.extend(quote!{T: crate::common::message::StaticDomainSenderSubscribe<#static_channel>,});
						fields_clone.extend(quote!{#field_name: <T as crate::common::message::StaticDomainSenderSubscribe<#static_channel>>::sender_subscribe(parent, &builder.static_fields.#domain_suffixed)
							.map_err(|e| crate::common::message::DomainSubscribeErr::NoDomain(format!("{e:#?}")))?
							.into(), });
					}
					SubsetKind::Receiver => {
						impls.extend(quote!{
							impl crate::common::message::HasReceiver<#static_channel> for #struct_ident { 
								fn get_receiver(&self) -> &#ty { 
									&self.#field_name
								}
							}
						});
						where_args.extend(quote!{T: crate::common::message::StaticDomainReceiverSubscribe<#static_channel>,});
						fields_clone.extend(quote!{#field_name: <T as crate::common::message::StaticDomainReceiverSubscribe<#static_channel>>::receiver_subscribe(parent, &builder.static_fields.#domain_suffixed)
							.map_err(|e| crate::common::message::DomainSubscribeErr::NoDomain(format!("{e:#?}")))?
							.into(), });
					}
				}
			}
			impls.extend(quote!{
				impl<T> crate::common::message::CloneComplexSubset<T> for #struct_ident where #where_args {
					fn from_subset_builder(parent: &T, builder: crate::common::message::SubsetBuilder<#builder_ident>) -> Result<Self, crate::common::message::DomainSubscribeErr<String>> {
						Ok(#struct_ident {
							#fields_clone
						})
					}
				}
				// Helper method - less boilerplate for building a channel-set from subset.
				impl From<#builder_ident> for crate::common::message::SubsetBuilder<#builder_ident> { 
    				fn from(value: #builder_ident) -> Self { 
						crate::common::message::SubsetBuilder::new(value)
					}
				}
			});
		}
		impls.into()
	}
	else { 
		panic!("Cannot use #[derive(ChannelSet)] on non-structs!")
	}
}
