//! Validação dos destinos de requisições HTTP para fontes públicas.
use std::time::Duration;

pub fn public_address(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || a == 0
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (b == 18 || b == 19))
                || (a == 192 && b == 0 && c == 0))
        }
        std::net::IpAddr::V6(ip) => {
            let s = ip.segments();
            (0x2000..=0x3fff).contains(&s[0])
                && !(s[0] == 0x2001 && (s[1] <= 0x1ff || s[1] == 0xdb8))
                && !(s[0] == 0x2002)
                && !(s[0] == 0x3fff)
        }
    }
}
/// Valida DNS e fixa os endereços; sem proxy ou redirects que contornem a validação.
pub async fn public_client(url: &str) -> anyhow::Result<reqwest::Client> {
    let parsed = reqwest::Url::parse(url)?;
    anyhow::ensure!(
        matches!(parsed.scheme(), "https" | "http")
            && parsed.username().is_empty()
            && parsed.password().is_none(),
        "URL HTTP inválida"
    );
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("URL sem host"))?;
    let addresses: Vec<_> =
        tokio::net::lookup_host((host, parsed.port_or_known_default().unwrap_or(443)))
            .await?
            .collect();
    anyhow::ensure!(
        !addresses.is_empty() && addresses.iter().all(|a| public_address(a.ip())),
        "Destino interno ou reservado não permitido"
    );
    Ok(reqwest::Client::builder()
        .no_proxy()
        .resolve_to_addrs(host, &addresses)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent("RadarCivico/1.0")
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_internal_and_reserved_addresses() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "::ffff:127.0.0.1",
            "fe80::1",
            "2001:db8::1",
        ] {
            assert!(!public_address(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_address("8.8.8.8".parse().unwrap()));
    }
}
