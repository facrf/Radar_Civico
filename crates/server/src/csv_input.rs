//! Conversão ISO-8859-1 em blocos limitados, sem carregar o CSV ou ZIP inteiro.
use std::io::{self, Read};

pub struct CsvInput<R> {
    inner: R,
    latin1: bool,
    converted: Vec<u8>,
    position: usize,
}
impl<R: Read> CsvInput<R> {
    pub fn new(inner: R, encoding: &str) -> io::Result<Self> {
        let latin1 = match encoding {
            "UTF-8" => false,
            "ISO-8859-1" => true,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Codificação inválida: use UTF-8 ou ISO-8859-1",
                ))
            }
        };
        Ok(Self {
            inner,
            latin1,
            converted: Vec::with_capacity(8192),
            position: 0,
        })
    }
}
impl<R: Read> Read for CsvInput<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if !self.latin1 || output.is_empty() {
            return self.inner.read(output);
        }
        if self.position == self.converted.len() {
            let mut input = [0u8; 4096];
            let size = self.inner.read(&mut input)?;
            self.converted.clear();
            self.position = 0;
            for &byte in &input[..size] {
                if byte < 128 {
                    self.converted.push(byte);
                } else {
                    self.converted.push(0xc0 | (byte >> 6));
                    self.converted.push(0x80 | (byte & 0x3f));
                }
            }
        }
        let size = output.len().min(self.converted.len() - self.position);
        output[..size].copy_from_slice(&self.converted[self.position..self.position + size]);
        self.position += size;
        Ok(size)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latin1_preserva_acentos_com_saidas_de_um_byte() {
        let bytes = b"NOME;VALOR\nJo\xe3o Gon\xe7alves;10\n";
        let mut reader = CsvInput::new(bytes.as_slice(), "ISO-8859-1").unwrap();
        let mut output = Vec::new();
        let mut chunk = [0u8; 1];
        while reader.read(&mut chunk).unwrap() != 0 {
            output.push(chunk[0]);
        }
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "NOME;VALOR\nJoão Gonçalves;10\n"
        );
        assert!(CsvInput::new(bytes.as_slice(), "AUTO").is_err());
    }
    #[test]
    fn utf8_preserva_texto_e_erros_do_leitor() {
        let mut reader = CsvInput::new("São Paulo".as_bytes(), "UTF-8").unwrap();
        let mut text = String::new();
        reader.read_to_string(&mut text).unwrap();
        assert_eq!(text, "São Paulo");
        struct Failed;
        impl Read for Failed {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("leitura interrompida"))
            }
        }
        assert!(CsvInput::new(Failed, "ISO-8859-1")
            .unwrap()
            .read(&mut [0; 2])
            .is_err());
    }
}
