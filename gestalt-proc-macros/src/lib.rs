#![feature(string_remove_matches)]

use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro2::TokenTree;
use quote::{quote, ToTokens, format_ident};
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, DeriveInput, Ident, LitInt, MetaList, Token, Type};
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


const CHANNEL_STR: &'static str = "channel";
const SENDER_STR: &'static str = "sender";
const RECEIVER_STR: &'static str = "receiver";
const TAKE_RECEIVER_STR: &'static str = "take_receiver";

const NEW_CHANNEL_STR: &'static str = "new_channel";
const MANUAL_INIT_STR: &'static str = "manual_init";

const DOMAIN_STR: &'static str = "domain";
const DOMAIN_SUFFIX: &'static str = "_domain";
const STATIC_BUILDER_SUFFIX: &'static str = "Fields";

#[derive(Clone, PartialEq)]
enum SubsetKind {
	Channel,
	Sender,
	Receiver,
	TakeReceiver,
}
impl SubsetKind {
	fn from_attr(attribute_parsed: &str) -> Option<Self> {
		// Check to see if this is *our* attribute and not something else.
		if attribute_parsed.contains(TAKE_RECEIVER_STR) { 
			Some(Self::TakeReceiver)
		}
		else if attribute_parsed.contains(RECEIVER_STR) {
			Some(Self::Receiver)
		}
		else if attribute_parsed.contains(SENDER_STR) {
			Some(Self::Sender)
		}
		else if attribute_parsed.contains(CHANNEL_STR) {
			Some(Self::Channel)
		}
		else {
			None
		}
	}
}

/// Non-channel-defining field attributes such as new_channel
#[derive(Clone, PartialEq)]
enum ChannelInitKind {
	/// Require this field to fall through into manual init (in the SubsetBuilder), 
	/// even if it's mapped to a channel. 
	ManualInit,
	/// Use ChannelInit::new() to build this one.
	/// This allows others to clone subsets from this
	/// channel set even after this channel set is cloned as a subset from others. 
	NewChannel,
}
impl ChannelInitKind {
	fn from_attr(attribute_parsed: &str) -> Option<Self> {
		// Check to see if this is *our* attribute and not something else.
		if attribute_parsed.contains(MANUAL_INIT_STR) { 
			Some(Self::ManualInit)
		}
		else if attribute_parsed.contains(NEW_CHANNEL_STR) {
			Some(Self::NewChannel)
		}
		else {
			None
		}
	}
}


/// Parses attributes such as `#[channel]` `#[receiver]`, `#[sender]`, etc
#[derive(Clone)]
struct ChannelHeader {
	pub static_channel: Ident,
	pub subset_kind: SubsetKind,
	/// Holds the suffixed domain field rather than domain_ty.
	pub domain: Option<Ident>,
	pub init_kind: Option<ChannelInitKind>,
}
impl ChannelHeader {
	pub fn from_attr(meta: &MetaList) -> Option<Self> {
		let attribute_parsed = meta.path.segments.last().unwrap().ident.to_string();
		let subset_kind = SubsetKind::from_attr(&attribute_parsed)?;

		let mut iter = meta.tokens.clone().into_iter();
		let first_token = iter.next()?; // There should *at least* be one.
		if let TokenTree::Ident(channel_ident) = &first_token {
			let mut prev_token = first_token.clone();
			let mut domain: Option<Ident> = None;
			let mut init_kind: Option<ChannelInitKind> = None;
			// Equality assignment begun, next token is the value
			let mut assigning_domain = false;
			while let Some(token) = iter.next() {
				let prev_token_string = prev_token.to_string();
				let token_string = token.to_string();
				if domain.is_some() && token_string.ends_with(DOMAIN_STR) { 
					panic!("Can only define one domain field per channel!");
				}
				if let Some(init) = ChannelInitKind::from_attr(&token_string.to_lowercase()) { 
					if init_kind.is_some() { 
						panic!("Cannot declare a channel new_channel and manual_init at the same time!");
					}
					else { 
						init_kind = Some(init);
					}
				}
				match &token {
					TokenTree::Punct(punct) => match punct.as_char() {
						'=' | ':' => {
							if prev_token_string.to_lowercase().ends_with(DOMAIN_STR) { 
								assigning_domain = true
							}
						}
						_ => {}, // Skip, separator-ness is already implicit in being tokenized.
					},
					TokenTree::Literal(literal) => {
						prev_token = token.clone();
						if assigning_domain {
							let mut domain_ident = literal.to_string();
							domain_ident.remove_matches("\"");
							domain_ident.remove_matches("\'");
							let domain_suffixed = format_ident!("{domain_ident}{DOMAIN_SUFFIX}");
							domain = Some(domain_suffixed);
							assigning_domain = false;
						}
					},
					_ => {
						prev_token = token.clone();
						if assigning_domain {
							let mut domain_ident = token_string.clone();
							domain_ident.remove_matches("\"");
							domain_ident.remove_matches("\'");
							let domain_suffixed = format_ident!("{domain_ident}{DOMAIN_SUFFIX}");
							domain = Some(domain_suffixed);
							assigning_domain = false;
						}
					}
				}
			}
			//Make sure we're not attempting to do something extremely nonsensical.
			if (init_kind == Some(ChannelInitKind::NewChannel)) && (subset_kind != SubsetKind::Channel) { 
				panic!("Cannot impl for {channel_ident:#?}: new_channel may only be used on a field that holds a channel, not a receiver or a sender.")
			}
			Some(Self{
				static_channel: channel_ident.clone(),
				subset_kind,
				domain,
				init_kind,
			})
		}
		else { 
			panic!("Non-ident for channel field!");
		}
	}
}

struct IdentifiedChannel {
	pub field_name: Ident,
	pub ty: Type,
	pub header: ChannelHeader
}

impl IdentifiedChannel {
	pub fn has_impl(&self, channel_already_impl: &mut HashSet<Ident>, struct_ident: &Ident) -> Option<proc_macro2::TokenStream> {
		let static_channel = &self.header.static_channel;
		if self.header.subset_kind == SubsetKind::Channel {
			if channel_already_impl.contains(static_channel) { 
					return None;
			}
		}

		let field_name = &self.field_name;
		let static_channel = &self.header.static_channel;
		Some(match self.header.subset_kind {
			SubsetKind::Channel => quote!{
				impl crate::common::message::HasChannel<#static_channel> for #struct_ident {
					fn get_channel(&self) -> &<#static_channel as crate::common::message::StaticChannelAtom>::Channel {
						&self.#field_name
					}
				}
			},
			SubsetKind::Sender => quote!{
				impl crate::common::message::StaticSenderSubscribe<#static_channel> for #struct_ident where #static_channel: crate::common::message::StaticChannelAtom, <#static_channel as crate::common::message::StaticChannelAtom>::Channel: SenderChannel<<#static_channel as crate::common::message::StaticChannelAtom>::Message>, <#static_channel as crate::common::message::StaticChannelAtom>::Message: Clone, <<#static_channel as crate::common::message::StaticChannelAtom>::Channel as crate::common::message::SenderChannel<<#static_channel as crate::common::message::StaticChannelAtom>::Message>>::Sender: Clone { 
					fn sender_subscribe(&self) -> <<#static_channel as crate::common::message::StaticChannelAtom>::Channel as crate::common::message::SenderChannel<<#static_channel as crate::common::message::StaticChannelAtom>::Message>>::Sender {
						self.#field_name.clone()
					}
				}
			},
			SubsetKind::Receiver => quote!{
				impl crate::common::message::HasReceiver<#static_channel> for #struct_ident { 
					fn get_receiver(&self) -> &<<#static_channel as crate::common::message::StaticChannelAtom>::Channel as crate::common::message::ReceiverChannel<<#static_channel as crate::common::message::StaticChannelAtom>::Message>>::Receiver {
						&self.#field_name
					}
				}
			},
			SubsetKind::TakeReceiver => { 
				return None;
			}
		})
	}
	/// Constraints to go in the Where clause in `impl FromSubset<T> for OurChannelSet where ...`
	pub fn t_constraint_impl(&self) -> Option<proc_macro2::TokenStream> {
		if self.header.init_kind.is_some() {
			return None;
		}
		let static_channel = &self.header.static_channel;
		Some(match (&self.header.subset_kind, self.header.domain.is_some()) {
			(SubsetKind::Channel, _) => {
				quote!{T: crate::common::message::HasChannel<#static_channel>,}
			},
			(SubsetKind::Sender, true) => {
				quote!{T: crate::common::message::StaticDomainSenderSubscribe<#static_channel>,}
			},
			(SubsetKind::Sender, false) => {
				quote!{T: crate::common::message::StaticSenderSubscribe<#static_channel>,}
			},
			(SubsetKind::Receiver, true) => {
				quote!{T: crate::common::message::StaticDomainReceiverSubscribe<#static_channel>,}
			},
			(SubsetKind::Receiver, false) => {
				quote!{T: crate::common::message::StaticReceiverSubscribe<#static_channel>,}
			},
			(SubsetKind::TakeReceiver, true) => { 
				quote!{T: crate::common::message::StaticDomainTakeReceiver<#static_channel>,}
			},
			(SubsetKind::TakeReceiver, false) => { 
				quote!{T: crate::common::message::StaticTakeReceiver<#static_channel>,}
			},
		})
	}
	pub fn static_builder_field(&self, domain_already_impl: &mut HashSet<Ident>) -> Option<proc_macro2::TokenStream> {
		let field_name = &self.field_name;
		let field_ty = &self.ty;
		match &self.header.init_kind {
			// ManualInit is the "force builder field" option.
			Some(ChannelInitKind::ManualInit) => Some(
				quote!{pub #field_name: #field_ty,}
			),
			// NewChannel ensures we initialize the channel new every time.
			Some(ChannelInitKind::NewChannel) => None,
			None => {
				let static_channel = &self.header.static_channel;
				if let Some(domain) = self.header.domain.as_ref() {
					if domain_already_impl.contains(domain) { 
						return None;
					}
					else {
						domain_already_impl.insert(domain.clone());
					}
				}
				self.header.domain.as_ref().map(|inner_value| {
					quote!{pub #inner_value: <#static_channel as crate::common::message::StaticDomainChannelAtom>::Domain,}
				})
			},
		}
	}
	pub fn new_channel_call(&self) -> Option<proc_macro2::TokenStream> { 
		if self.header.init_kind == Some(ChannelInitKind::NewChannel) {
			let field_name = &self.field_name;
			let static_channel = &self.header.static_channel;
			Some(
				quote!{#field_name: <<#static_channel as crate::common::message::StaticChannelAtom>::Channel as ChannelInit>::new(builder.capacity_conf.get_or_default::<#static_channel>()),}
			)
		} else {
			None
		}
	}
	pub fn init_line(&self) -> proc_macro2::TokenStream {
		let field_name = &self.field_name;
		if self.header.init_kind == Some(ChannelInitKind::ManualInit) {
			return quote!{#field_name: builder.static_fields.#field_name,};
		}
		let static_channel = &self.header.static_channel;
		if self.header.init_kind == Some(ChannelInitKind::NewChannel) {
			let new_channel_call = self.new_channel_call().unwrap();
			return new_channel_call;
		}
		match (&self.header.subset_kind, self.header.domain.as_ref()) {
			(SubsetKind::Channel, _) => quote!{
				#field_name: <T as crate::common::message::HasChannel<#static_channel>>::get_channel(parent).clone().into(),
			},
			(SubsetKind::Sender, None) => quote!{
				#field_name: <T as crate::common::message::StaticSenderSubscribe<#static_channel>>::sender_subscribe(parent).into(),
			},
			(SubsetKind::Sender, Some(domain)) => quote!{
				#field_name: <T as crate::common::message::StaticDomainSenderSubscribe<#static_channel>>::sender_subscribe(parent, &builder.static_fields.#domain)
					.map_err(|e| e.to_string_form())?
					.into(),
			},
			(SubsetKind::Receiver, None) => quote!{
				#field_name: <T as crate::common::message::StaticReceiverSubscribe<#static_channel>>::receiver_subscribe(parent).into(),
			},
			(SubsetKind::Receiver, Some(domain)) => quote!{
				#field_name: <T as crate::common::message::StaticDomainReceiverSubscribe<#static_channel>>::receiver_subscribe(parent, &builder.static_fields.#domain)
					.map_err(|e| e.to_string_form())?
					.into(),
			},
			(SubsetKind::TakeReceiver, None) => quote!{
				#field_name: <T as crate::common::message::StaticTakeReceiver<#static_channel>>::take_receiver(parent)?
					.into(),
			},
			(SubsetKind::TakeReceiver, Some(domain)) => quote!{
				#field_name: <T as crate::common::message::StaticDomainTakeReceiver<#static_channel>>::take_receiver(parent, &builder.static_fields.#domain)
					.map_err(|e| e.to_string_form())?
					.into(),
			},
		}
	}
	pub fn requires_subset(&self) -> bool {
		self.header.init_kind.is_none()
	}
}

#[proc_macro_derive(ChannelSet, attributes(channel, sender, receiver, take_receiver))]
pub fn impl_channel_set(channel_set: TokenStream) -> TokenStream {
	let parsed = parse_macro_input!(channel_set as DeriveInput);
	
	let struct_ident = parsed.ident.clone();

	if let syn::Data::Struct(struct_data) = parsed.data {
		// Field lines for from_subset()
		let mut subset_field_entries = proc_macro2::TokenStream::new();
		// remains false if every field can be initialized new or from static_fields
		let mut requires_subset = false;
		
		// For subset builder stuff such as domains.
		let mut static_builder_fields = Vec::new();
		// The actual thing we're trying to build here.
		let mut impls: proc_macro2::TokenStream = proc_macro2::TokenStream::new();
		// Also do our ridiculous where clause.
		let mut where_args = proc_macro2::TokenStream::new();
		let mut at_least_one_new = false;

		let mut channel_already_impl: HashSet<Ident> = HashSet::new();

		let mut domain_already_impl: HashSet<Ident> = HashSet::new();
		// Loop through, appending each HasChannel impl to our implementations.
		// Find fields with #[channel(T)] attributes
		for field in struct_data.fields.iter() {
			let mut non_channel = true;

			let field_ty = &field.ty;
			let field_ident = field.ident.as_ref().unwrap();

			for attr in field.attrs.iter() {
				if attr.meta.path().segments.len() == 0 {
					continue;
				}
				let meta = match attr.meta.require_list() { 
					Ok(m) => m,
					Err(_) => { continue; }
				};
				// Check to see if this is *our* attribute and not something else.
				if let Some(header) = ChannelHeader::from_attr(meta) {
					non_channel = false;
					let identified_channel = IdentifiedChannel {
						field_name: field_ident.clone(),
						header,
						ty: field.ty.clone(),
					};
					// Our part of static_fields
					if let Some(value) = identified_channel.static_builder_field(&mut domain_already_impl) { 
						static_builder_fields.push(value);
					}
					if identified_channel.requires_subset() { 
						requires_subset = true;
					}
					// Implement HasChannel<> and such on our set.
					if let Some(value) = identified_channel.has_impl(&mut channel_already_impl, &struct_ident) {
						impls.extend(value);
					}
					// Extend where constraints for clone subset
					if let Some(value) = identified_channel.t_constraint_impl() { 
						where_args.extend(value);
					}
					if identified_channel.new_channel_call().is_some() { 
						at_least_one_new = true;
					}
					// Actual CloneSubset behavior
					subset_field_entries.extend(identified_channel.init_line());
				}
			}
			//None of our attributes? Do this instead.
			if non_channel {
				let type_string = field_ty.to_token_stream().to_string();

				// Make sure we don't force people to muck around with PhantomData in builders.
				if !type_string.contains("PhantomData") {
					static_builder_fields.push(quote!{pub #field_ident: #field_ty,});
					subset_field_entries.extend(quote!{#field_ident: builder.static_fields.#field_ident,});
				}
				else {
					subset_field_entries.extend(quote!{#field_ident: std::marker::PhantomData});
				}
			}
		}
		let no_builder_fields = static_builder_fields.is_empty();
		let static_builder_fields = proc_macro2::TokenStream::from_iter(static_builder_fields.into_iter());
		let builder_ident =  if no_builder_fields {
			quote!{()}
		} else {
			let builder_ident_inner = format_ident!("{struct_ident}{STATIC_BUILDER_SUFFIX}");
			impls.extend(quote!{
				pub struct #builder_ident_inner {
					#static_builder_fields
				}
				impl From<#builder_ident_inner> for crate::common::message::SubsetBuilder<#builder_ident_inner> { 
					fn from(value: #builder_ident_inner) -> Self { 
						crate::common::message::SubsetBuilder::new(value)
					}
				}
			});
			quote!{#builder_ident_inner}
		};
		impls.extend(quote!{
			impl crate::common::message::ChannelSet for #struct_ident { 
				type StaticBuilder = #builder_ident;
			}
			impl<T> crate::common::message::CloneSubset<T> for #struct_ident where #where_args {
				fn build_from(parent: &T, builder: crate::common::message::SubsetBuilder<#builder_ident>) 
						-> Result<Self, crate::common::message::DomainSubscribeErr<String>> {
					Ok(#struct_ident {
						#subset_field_entries
					})
				}
			}
		});
		if at_least_one_new && !requires_subset { 
			impls.extend(quote!{
				impl #struct_ident { 
					pub fn new(builder: crate::common::message::SubsetBuilder<#builder_ident>) -> Self {
						Self { 
							#subset_field_entries
						}
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
