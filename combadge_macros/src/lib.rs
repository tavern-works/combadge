extern crate proc_macro;
use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn combadge(attr: TokenStream, item: TokenStream) -> TokenStream {
    println!("attr: {attr}");
    println!("item: {item}");
    item
}
