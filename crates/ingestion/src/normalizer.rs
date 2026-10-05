use encoding_rs::WINDOWS_1252;

pub fn limpar_apenas_digitos(input: &str) -> String {
    input.chars().filter(|c| c.is_ascii_digit()).collect()
}

pub fn limpar_cnpj(cnpj: &str) -> String {
    limpar_apenas_digitos(cnpj)
}

pub fn extrair_cnpj_raiz(cnpj: &str) -> String {
    let clean = limpar_cnpj(cnpj);
    if clean.len() >= 8 {
        clean[..8].to_string()
    } else {
        clean
    }
}

pub fn mascarar_cpf(cpf: &str) -> String {
    // Caso venha mascarado do TSE (ex: ***123456** ou ***.123.456-**)
    if cpf.contains('*') {
        let miolo: String = cpf.chars().filter(|c| c.is_ascii_digit()).collect();
        if miolo.len() == 6 {
            return format!("***.{}.{}-**", &miolo[0..3], &miolo[3..6]);
        }
    }

    let digitos = limpar_apenas_digitos(cpf);
    if digitos.len() == 11 {
        // Formato padrão brasileiro: ***.DDD.DDD-**
        format!("***.{}.{}-**", &digitos[3..6], &digitos[6..9])
    } else if digitos.len() == 6 {
        format!("***.{}.{}-**", &digitos[0..3], &digitos[3..6])
    } else {
        cpf.trim().to_string()
    }
}

pub fn converter_latin1_para_utf8(bytes: &[u8]) -> String {
    let (cow, _encoding, _had_errors) = WINDOWS_1252.decode(bytes);
    cow.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalizer_mascaramento_cpf() {
        // CPF completo (11 dígitos)
        let cpf_completo = "123.456.789-01";
        assert_eq!(mascarar_cpf(cpf_completo), "***.456.789-**");

        let cpf_sem_pontos = "12345678901";
        assert_eq!(mascarar_cpf(cpf_sem_pontos), "***.456.789-**");

        // CPF já pré-mascarado do TSE
        let cpf_tse = "***456789**";
        assert_eq!(mascarar_cpf(cpf_tse), "***.456.789-**");

        let cpf_tse_pontuado = "***.456.789-**";
        assert_eq!(mascarar_cpf(cpf_tse_pontuado), "***.456.789-**");
    }

    #[test]
    fn test_normalizer_cnpj_e_raiz() {
        let cnpj_formatado = "12.345.678/0001-90";
        assert_eq!(limpar_cnpj(cnpj_formatado), "12345678000190");
        assert_eq!(extrair_cnpj_raiz(cnpj_formatado), "12345678");

        let cnpj_sujo = " 12.345.678/0001-90 \n";
        assert_eq!(limpar_cnpj(cnpj_sujo), "12345678000190");
        assert_eq!(extrair_cnpj_raiz(cnpj_sujo), "12345678");
    }

    #[test]
    fn test_normalizer_latin1_para_utf8() {
        // Bytes em ISO-8859-1 / Windows-1252 para "ELEIÇÃO MUNICIPAL DE SÃO PAULO"
        // 'Ç' em ISO-8859-1 é 0xC7
        // 'Ã' em ISO-8859-1 é 0xC3
        let bytes_iso: Vec<u8> = vec![
            b'E', b'L', b'E', b'I', 0xC7, 0xC3, b'O', b' ', b'M', b'U', b'N', b'I', b'C', b'I',
            b'P', b'A', b'L',
        ];

        let utf8_str = converter_latin1_para_utf8(&bytes_iso);
        assert_eq!(utf8_str, "ELEIÇÃO MUNICIPAL");
    }
}
