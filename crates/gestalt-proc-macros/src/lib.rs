use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Ident, Token, LitInt};

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
        }
        else if input.peek(Ident) {
            id_ident = input.parse()?;
        }
        else {
            return Err(input.error("Expected literal integer or identifier"));
        }
        input.parse::<Token![,]>()?;
        let sidedness = input.parse()?;
        input.parse::<Token![,]>()?;
        let guarantee = input.parse()?;
        let stream_select = if !input.is_empty() {
            input.parse::<Token![,]>()?;
            Some(input.parse()?)
        } else { None };
        Ok(NetMsgAttr {
            id_lit, id_ident, sidedness, guarantee, stream_select
        })
    }
}


#[proc_macro_attribute]
pub fn netmsg(attr: TokenStream, item: TokenStream) -> TokenStream {
    let NetMsgAttr { id_lit, id_ident, sidedness, guarantee, stream_select } = parse_macro_input!(attr as NetMsgAttr);
    let stream_select = match stream_select {
        Some(s) => quote! { crate::net::netmsg::StreamSelector::Specific(#s) },
        None => quote! { crate::net::netmsg::StreamSelector::Any }
    };

    let id = if let Some(i) = id_lit { quote!(#i) }
    else if let Some(i) = id_ident { quote!(#i) }
    else { unreachable!() };

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

impl Into<crate::net::netmsg::PacketIntermediary> for &#message {
    fn into(self) -> crate::net::netmsg::PacketIntermediary {
        use crate::net::netmsg::NetMsg;
        self.construct_packet().unwrap()
    }
}
    }).into()
}
