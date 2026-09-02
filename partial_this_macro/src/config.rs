use proc_macro2::Span;
use syn::{
    Expr, ExprLit, Ident, ItemStruct, Lit, Meta, Token,
    parse::{Parse, ParseStream},
};

/// The crate name used by default in generated paths.
const DEFAULT_CRATE_NAME: &str = "partial_this";

/// Config parsed from the `#[partial(...)]` attribute arguments.
///
/// The syntax is `key = value`, with multiple configs separated by commas, e.g.
/// `#[partial(module = foo, crate_name = my_crate, pub_use = false)]`.
#[derive(Debug, Default)]
pub(crate) struct PartialConfig {
    /// Name of the module that holds the generated output.
    module: Option<Ident>,
    /// Name of the crate that exposes `ThisPtr`/`UninitThis`/`typenum`.
    ///
    /// Defaults to `partial_this`; set it to the dependency alias when the
    /// `partial_this` crate is renamed in `Cargo.toml`.
    crate_name: Option<Ident>,
    /// Whether to emit the `use`/`pub use` re-exports of the generated builder
    /// and accessor traits. Defaults to `true`.
    pub_use: Option<bool>,
}

impl PartialConfig {
    /// Returns the name of the generated module, deriving a default from the
    /// struct name when the module is not configured.
    pub(crate) fn module_name(&self, item: &ItemStruct) -> Ident {
        let name = self.module.clone().unwrap_or_else(|| {
            let struct_name = &item.ident;
            let snake = to_snake_case(&struct_name.to_string());
            Ident::new(&format!("{snake}_partial"), struct_name.span())
        });
        name
    }

    /// Returns the crate name used to build absolute paths in the generated code.
    pub(crate) fn crate_name(&self) -> Ident {
        self.crate_name
            .clone()
            .unwrap_or_else(|| Ident::new(DEFAULT_CRATE_NAME, Span::call_site()))
    }

    /// Whether the generated builder/accessor traits should be re-exported.
    pub(crate) fn pub_use(&self) -> bool {
        self.pub_use.unwrap_or(true)
    }
}

impl Parse for PartialConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let metas = input.parse_terminated(Meta::parse, Token![,])?;
        let mut config = PartialConfig::default();

        for meta in metas {
            let Meta::NameValue(nv) = meta else {
                return Err(syn::Error::new_spanned(&meta, "expected `key = value`"));
            };
            let Some(key) = nv.path.get_ident() else {
                return Err(syn::Error::new_spanned(
                    &nv.path,
                    "config key must be a plain identifier",
                ));
            };

            match key.to_string().as_str() {
                "module" => config.module = Some(parse_ident(&nv.value)?),
                "crate_name" => config.crate_name = Some(parse_ident(&nv.value)?),
                "pub_use" => config.pub_use = Some(parse_bool(&nv.value)?),
                _ => {
                    return Err(syn::Error::new_spanned(
                        key,
                        format!("unknown config key `{key}`"),
                    ));
                }
            }
        }

        Ok(config)
    }
}

/// Parses a boolean literal (`true`/`false`).
fn parse_bool(expr: &Expr) -> syn::Result<bool> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Bool(b), ..
        }) => Ok(b.value),
        _ => Err(syn::Error::new_spanned(expr, "expected `true` or `false`")),
    }
}

/// Parses an identifier, accepting either a bare identifier (e.g. `foo`) or a
/// string literal (e.g. `"foo"`).
fn parse_ident(expr: &Expr) -> syn::Result<Ident> {
    match expr {
        Expr::Path(path) if path.path.segments.len() == 1 => {
            Ok(path.path.segments[0].ident.clone())
        }
        Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) => syn::parse_str::<Ident>(&s.value())
            .map_err(|e| syn::Error::new(s.span(), e.to_string())),
        _ => Err(syn::Error::new_spanned(
            expr,
            "expected an identifier or string literal",
        )),
    }
}

/// Converts a PascalCase struct name to snake_case.
fn to_snake_case(name: &str) -> String {
    let mut snake = String::with_capacity(name.len() + 4);
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                snake.push('_');
            }
            snake.push(ch.to_ascii_lowercase());
        } else {
            snake.push(ch);
        }
    }
    snake
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse2;

    fn parse(input: &str) -> PartialConfig {
        parse2::<PartialConfig>(input.parse().unwrap()).unwrap()
    }

    #[test]
    fn parses_module_ident() {
        let cfg = parse("module = foo");
        assert_eq!(cfg.module.unwrap().to_string(), "foo");
    }

    #[test]
    fn parses_module_string() {
        let cfg = parse("module = \"foo_bar\"");
        assert_eq!(cfg.module.unwrap().to_string(), "foo_bar");
    }

    #[test]
    fn parses_empty() {
        let cfg = parse("");
        assert!(cfg.module.is_none());
    }

    #[test]
    fn rejects_unknown_key() {
        let result = parse2::<PartialConfig>("unknown = x".parse().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn rejects_non_key_value() {
        let result = parse2::<PartialConfig>("foo".parse().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn parses_crate_name_ident() {
        let cfg = parse("crate_name = my_alias");
        assert_eq!(cfg.crate_name().to_string(), "my_alias");
    }

    #[test]
    fn parses_crate_name_string() {
        let cfg = parse("crate_name = \"my_alias\"");
        assert_eq!(cfg.crate_name().to_string(), "my_alias");
    }

    #[test]
    fn defaults_crate_name() {
        let cfg = parse("");
        assert_eq!(cfg.crate_name().to_string(), "partial_this");
    }

    #[test]
    fn defaults_pub_use_true() {
        let cfg = parse("");
        assert!(cfg.pub_use());
    }

    #[test]
    fn parses_pub_use_false() {
        let cfg = parse("pub_use = false");
        assert!(!cfg.pub_use());
    }

    #[test]
    fn derives_default_module_name() {
        let item: ItemStruct = syn::parse_str("struct Foo { x: i32 }").unwrap();
        let cfg = parse("");
        assert_eq!(cfg.module_name(&item).to_string(), "foo_partial");
    }
}
