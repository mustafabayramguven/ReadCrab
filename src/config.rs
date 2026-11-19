use std::{net::IpAddr, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub data_dir: PathBuf,
    pub books_dir: PathBuf,
    pub cookie_secure: bool,
    /// `X-Forwarded-For` başlığına yalnız bu adreslerden gelen isteklerde güvenilir.
    pub trusted_proxies: Vec<Cidr>,
}

/// IP ağı (`10.0.0.0/8`, `172.18.0.2`, `fd00::/8`); önek verilmezse tek adres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cidr {
    net: IpAddr,
    prefix: u8,
}

impl Cidr {
    pub fn parse(s: &str) -> Option<Cidr> {
        let (addr, prefix) = match s.trim().split_once('/') {
            Some((a, p)) => (a, Some(p)),
            None => (s.trim(), None),
        };
        let net: IpAddr = addr.trim().parse().ok()?;
        let max = if net.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix {
            Some(p) => p.trim().parse::<u8>().ok().filter(|&p| p <= max)?,
            None => max,
        };
        Some(Cidr { net, prefix })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.net, ip.to_canonical()) {
            (IpAddr::V4(n), IpAddr::V4(i)) => {
                let mask = u32::MAX
                    .checked_shl(32 - u32::from(self.prefix))
                    .unwrap_or(0);
                u32::from(n) & mask == u32::from(i) & mask
            }
            (IpAddr::V6(n), IpAddr::V6(i)) => {
                let mask = u128::MAX
                    .checked_shl(128 - u32::from(self.prefix))
                    .unwrap_or(0);
                u128::from(n) & mask == u128::from(i) & mask
            }
            _ => false,
        }
    }
}

/// `TRUSTED_PROXIES` değeri: virgülle ayrılmış adres/ağ listesi; boşsa hiçbir vekile güvenilmez.
pub fn parse_trusted_proxies(v: &str) -> Result<Vec<Cidr>, String> {
    v.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| Cidr::parse(s).ok_or_else(|| format!("Geçersiz TRUSTED_PROXIES girdisi: {s}")))
        .collect()
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let port = match std::env::var("PORT") {
            Ok(val) => val
                .parse::<u16>()
                .ok()
                .filter(|&p| p > 0)
                .ok_or_else(|| format!("Geçersiz PORT değeri: {val}"))?,
            Err(_) => 3435,
        };
        let data_dir = std::env::var("DATA_DIR")
            .unwrap_or_else(|_| "/data".to_string())
            .into();
        let books_dir = std::env::var("BOOKS_DIR")
            .unwrap_or_else(|_| "/books".to_string())
            .into();
        let cookie_secure = std::env::var("COOKIE_SECURE")
            .map(|v| v == "1")
            .unwrap_or(false);
        let trusted_proxies =
            parse_trusted_proxies(&std::env::var("TRUSTED_PROXIES").unwrap_or_default())?;

        Ok(Self {
            port,
            data_dir,
            books_dir,
            cookie_secure,
            trusted_proxies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cidr_matching() {
        let n = Cidr::parse("172.16.0.0/12").unwrap();
        assert!(n.contains("172.19.0.5".parse().unwrap()));
        assert!(n.contains("::ffff:172.19.0.5".parse().unwrap()));
        assert!(!n.contains("172.32.0.1".parse().unwrap()));
        let one = Cidr::parse(" 10.0.0.7 ").unwrap();
        assert!(one.contains("10.0.0.7".parse().unwrap()));
        assert!(!one.contains("10.0.0.8".parse().unwrap()));
        let all = Cidr::parse("0.0.0.0/0").unwrap();
        assert!(all.contains("8.8.8.8".parse().unwrap()));
        assert!(!all.contains("::1".parse().unwrap()));
        let v6 = Cidr::parse("fd00::/8").unwrap();
        assert!(v6.contains("fd12::1".parse().unwrap()));
        assert!(!v6.contains("fe80::1".parse().unwrap()));
        assert!(Cidr::parse("10.0.0.0/33").is_none());
        assert!(Cidr::parse("nas").is_none());
    }

    #[test]
    fn trusted_proxies_list() {
        assert_eq!(parse_trusted_proxies("").unwrap(), vec![]);
        assert_eq!(
            parse_trusted_proxies("10.0.0.1, 172.16.0.0/12")
                .unwrap()
                .len(),
            2
        );
        assert!(parse_trusted_proxies("10.0.0.1,xyz").is_err());
    }
}
