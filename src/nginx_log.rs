use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use chrono::{DateTime, Utc};
use winnow::Result;
use winnow::ascii::space0;
use winnow::combinator::{alt, delimited};
use winnow::token::{take_till, take_until};
use winnow::{Parser, ascii::digit1, combinator::separated};

fn main() -> anyhow::Result<()> {
    let s = r#"93.180.71.3 - - [17/May/2015:08:05:32 +0000] "GET /downloads/product_1 HTTP/1.1" 304 0 "-" "Debian APT-HTTP/1.3 (0.8.16~exp12ubuntu10.21)""#;
    let log = parse_nginx_log(s).map_err(|e| anyhow::anyhow!("Failed to parse log: {:?}", e))?;

    println!("{:?}", log);
    Ok(())
}

//93.180.71.3 - - [17/May/2015:08:05:32 +0000] "GET /downloads/product_1 HTTP/1.1" 304 0 "-" "Debian APT-HTTP/1.3 (0.8.16~exp12ubuntu10.21)"

#[allow(unused)]
#[derive(Debug)]
struct NginxLog {
    addr: IpAddr,
    datetime: DateTime<Utc>,
    method: HttpMethod,
    path: String,
    http_version: HttpVersion,
    status_code: u16,
    size: u64,
    referer: String,
    user_agent: String,
}

#[derive(Debug, PartialEq, Eq)]
enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Connect,
    Trace,
    Patch,
}

#[derive(Debug, PartialEq, Eq)]
enum HttpVersion {
    Http1_0,
    Http1_1,
    Http2_0,
    Http3_0,
}

//93.180.71.3 - - [17/May/2015:08:05:32 +0000] "GET /downloads/product_1 HTTP/1.1" 304 0 "-" "Debian APT-HTTP/1.3 (0.8.16~exp12ubuntu10.21)"
fn parse_nginx_log(input: &str) -> Result<NginxLog> {
    let input = &mut (&*input);
    let ip = parse_ip(input)?;
    parse_ignore(input)?;
    let datetime = parse_datetime(input)?;
    let (method, url, version) = parse_http(input)?;
    let status = parse_status(input)?;
    let body_bytes = parse_body_bytes(input)?;
    let referer = parse_quoted_string(input)?;
    let user_agent = parse_quoted_string(input)?;
    Ok(NginxLog {
        addr: ip,
        datetime,
        method,
        path: url,
        http_version: version,
        status_code: status,
        size: body_bytes,
        referer,
        user_agent,
    })
}

fn parse_ip(input: &mut &str) -> Result<IpAddr> {
    let digits: Vec<u8> = separated(4, digit1.parse_to::<u8>(), ".").parse_next(input)?;
    space0(input)?;
    Ok(IpAddr::V4(Ipv4Addr::new(
        digits[0], digits[1], digits[2], digits[3],
    )))
}

fn parse_ignore(input: &mut &str) -> Result<()> {
    "- - ".parse_next(input)?;
    Ok(())
}

fn parse_datetime(input: &mut &str) -> Result<DateTime<Utc>> {
    let datetime = delimited("[", take_till(0.., ']'), "]").parse_next(input)?;
    space0(input)?;
    Ok(DateTime::parse_from_str(datetime, "%d/%b/%Y:%H:%M:%S %z")
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap())
}

fn parse_http(input: &mut &str) -> Result<(HttpMethod, String, HttpVersion)> {
    let parse = (parse_http_method, parse_url, parse_http_version);
    let (method, url, version) = delimited('"', parse, '"').parse_next(input)?;
    space0(input)?;
    Ok((method, url, version))
}

fn parse_http_method(input: &mut &str) -> Result<HttpMethod> {
    let method = alt((
        "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "CONNECT", "TRACE", "PATCH",
    ))
    .parse_to()
    .parse_next(input)?;
    space0(input)?;
    Ok(method)
}

fn parse_url(input: &mut &str) -> Result<String> {
    let url = take_till(1.., ' ').parse_next(input)?;
    space0(input)?;
    Ok(url.to_string())
}

fn parse_http_version(input: &mut &str) -> Result<HttpVersion> {
    let version = alt(("HTTP/1.0", "HTTP/1.1", "HTTP/2.0", "HTTP/3.0"))
        .parse_to()
        .parse_next(input)?;
    space0(input)?;
    Ok(version)
}

fn parse_status(s: &mut &str) -> Result<u16> {
    let ret = digit1.parse_to().parse_next(s)?;
    space0(s)?;
    Ok(ret)
}

fn parse_body_bytes(s: &mut &str) -> Result<u64> {
    let ret = digit1.parse_to().parse_next(s)?;
    space0(s)?;
    Ok(ret)
}

fn parse_quoted_string(s: &mut &str) -> Result<String> {
    let ret = delimited('"', take_until(1.., '"'), '"').parse_next(s)?;
    space0(s)?;
    Ok(ret.to_string())
}

impl FromStr for HttpMethod {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(HttpMethod::Get),
            "POST" => Ok(HttpMethod::Post),
            "PUT" => Ok(HttpMethod::Put),
            "DELETE" => Ok(HttpMethod::Delete),
            "HEAD" => Ok(HttpMethod::Head),
            "OPTIONS" => Ok(HttpMethod::Options),
            "CONNECT" => Ok(HttpMethod::Connect),
            "TRACE" => Ok(HttpMethod::Trace),
            "PATCH" => Ok(HttpMethod::Patch),
            _ => Err(anyhow::anyhow!("Invalid http method")),
        }
    }
}

impl FromStr for HttpVersion {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "HTTP/1.0" => Ok(HttpVersion::Http1_0),
            "HTTP/1.1" => Ok(HttpVersion::Http1_1),
            "HTTP/2.0" => Ok(HttpVersion::Http2_0),
            "HTTP/3.0" => Ok(HttpVersion::Http3_0),
            _ => Err(anyhow::anyhow!("Invalid http version")),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn parse_ip_should_work() -> Result<()> {
        let mut s = "1.1.1.1";
        let ip = parse_ip(&mut s).unwrap();
        assert_eq!(s, "");
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));
        Ok(())
    }

    #[test]
    fn parse_datetime_should_work() -> Result<()> {
        let mut s = "[17/May/2015:08:05:32 +0000]";
        let dt = parse_datetime(&mut s).unwrap();
        assert_eq!(s, "");
        assert_eq!(dt, Utc.with_ymd_and_hms(2015, 5, 17, 8, 5, 32).unwrap());
        Ok(())
    }

    #[test]
    fn parse_http_should_work() -> Result<()> {
        let mut s = "\"GET /downloads/product_1 HTTP/1.1\"";
        let (method, url, protocol) = parse_http(&mut s).unwrap();
        assert_eq!(s, "");
        assert_eq!(method, HttpMethod::Get);
        assert_eq!(url, "/downloads/product_1");
        assert_eq!(protocol, HttpVersion::Http1_1);
        Ok(())
    }
}
