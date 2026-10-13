//! Cruzamentos por identidade: nomes e documentos mascarados são apenas indícios.
pub fn digits(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}
pub fn complete(value: &str) -> bool {
    if !value
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '/' | ' '))
    {
        return false;
    }
    let document = digits(value);
    let numbers: Vec<u32> = document.bytes().map(|b| u32::from(b - b'0')).collect();
    if !matches!(numbers.len(), 11 | 14) || numbers.iter().all(|n| *n == numbers[0]) {
        return false;
    }
    let base = numbers.len() - 2;
    for end in base..numbers.len() {
        let sum: u32 = numbers[..end]
            .iter()
            .enumerate()
            .map(|(index, number)| {
                let weight = if numbers.len() == 11 {
                    (end + 1 - index) as u32
                } else {
                    ((end - 1 - index) % 8 + 2) as u32
                };
                number * weight
            })
            .sum();
        let remainder = sum % 11;
        let check = if remainder < 2 { 0 } else { 11 - remainder };
        if numbers[end] != check {
            return false;
        }
    }
    true
}
/// Função determinística usada nos mesmos cruzamentos executados em SQL.
pub fn register_sql(conn: &storage::rusqlite::Connection) -> storage::Result<()> {
    use storage::rusqlite::functions::FunctionFlags;
    conn.create_scalar_function(
        "valid_document",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |context| {
            Ok(context
                .get::<Option<String>>(0)?
                .is_some_and(|doc| complete(&doc)))
        },
    )?;
    Ok(())
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
    // Documento completo inválido não pode virar um cruzamento por homônimo.
    if matches!(digits.len(), 11 | 14) || (!doc.trim().is_empty() && !doc.contains('*')) {
        return ("0".into(), vec![], false);
    }
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
    fn valida_digitos_verificadores_e_rejeita_placeholders() {
        for doc in ["529.982.247-25", "04.252.011/0001-10", "12311122207"] {
            assert!(complete(doc), "{doc}");
        }
        for doc in [
            "00000000000",
            "11111111111",
            "00000000000000",
            "52998224724",
            "04252011000111",
            "CPF52998224725",
            "",
            "***.982.247-**",
        ] {
            assert!(!complete(doc), "{doc}");
        }
        let (sql, values, confirmed) = predicate("cpf", "nome", "52998224724", "JOAO");
        assert_eq!(sql, "0");
        assert!(values.is_empty());
        assert!(!confirmed);
    }
    #[test]
    fn documentos_completos_nao_admitem_homonimos() {
        let (sql, values, confirmed) = predicate("cpf", "nome", "123.111.222-07", "JOAO");
        assert!(confirmed);
        assert!(!sql.contains(" OR "));
        assert_eq!(values, vec!["12311122207"]);
        let (sql, values, confirmed) = predicate("cpf", "nome", "***.111.222-**", "JOAO");
        assert!(!confirmed);
        assert!(sql.contains(" AND "));
        assert_eq!(values, vec!["111222", "JOAO"]);
    }
}
