//! Cruzamentos por identidade: nomes e documentos mascarados são apenas indícios.
pub fn digits(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}
pub fn complete(value: &str) -> bool {
    !value.contains('*') && matches!(digits(value).len(), 11 | 14)
}
pub fn document_sql(column: &str) -> String {
    format!("REPLACE(REPLACE(REPLACE(REPLACE({column},'.',''),'-',''),'/',''),' ','')")
}
/// A identidade completa não admite fallback por nome. Máscaras precisam de nome exato.
pub fn predicate(
    doc_column: &str,
    name_column: &str,
    doc: &str,
    name: &str,
) -> (String, Vec<String>, bool) {
    let column = document_sql(doc_column);
    if complete(doc) {
        return (format!("{column}=?"), vec![digits(doc)], true);
    }
    let digits = digits(doc);
    let middle = if digits.len() == 6 {
        Some(digits)
    } else {
        None
    };
    if let Some(middle) = middle {
        if !name.trim().is_empty() {
            return (
                format!("SUBSTR({column},4,6)=? AND TRIM(UPPER({name_column}))=?"),
                vec![middle, name.trim().to_uppercase()],
                false,
            );
        }
        // Uma máscara sem nome não identifica a pessoa procurada.
        return ("0".into(), vec![], false);
    }
    if !name.trim().is_empty() {
        return (
            format!("TRIM(UPPER({name_column}))=?"),
            vec![name.trim().to_uppercase()],
            false,
        );
    }
    ("0".into(), vec![], false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documentos_completos_nao_admitem_homonimos() {
        let (sql, values, confirmed) = predicate("cpf", "nome", "123.111.222-01", "JOAO");
        assert!(confirmed);
        assert!(!sql.contains(" OR "));
        assert_eq!(values, vec!["12311122201"]);
        let (sql, values, confirmed) = predicate("cpf", "nome", "***.111.222-**", "JOAO");
        assert!(!confirmed);
        assert!(sql.contains(" AND "));
        assert_eq!(values, vec!["111222", "JOAO"]);
    }
}
