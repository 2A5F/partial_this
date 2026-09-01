use syn::{
    Expr, ExprLit, Ident, ItemStruct, Lit, Meta, Token,
    parse::{Parse, ParseStream},
};

/// Config parsed from the `#[partial(...)]` attribute arguments.
///
/// The syntax is `key = value`, with multiple configs separated by commas, e.g.
/// `#[partial(module = foo)]`.
#[derive(Debug, Default)]
pub(crate) struct PartialConfig {
    /// Name of the module that holds the generated output.
    module: Option<Ident>,
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
                "module" => config.module = Some(parse_module_ident(&nv.value)?),
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

/// Parses a module name, accepting either a bare identifier (e.g. `foo`) or a
/// string literal (e.g. `"foo"`).
fn parse_module_ident(expr: &Expr) -> syn::Result<Ident> {
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
            "module name must be an identifier or string literal, \
             e.g. `module = foo` or `module = \"foo\"`",
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
    fn derives_default_module_name() {
        let item: ItemStruct = syn::parse_str("struct Foo { x: i32 }").unwrap();
        let cfg = parse("");
        assert_eq!(cfg.module_name(&item).to_string(), "foo_partial");
    }
}
