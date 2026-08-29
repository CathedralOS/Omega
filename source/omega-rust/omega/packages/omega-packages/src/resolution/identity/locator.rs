use super::{GitTransport, IdentityError};

#[derive(Debug)]
pub(super) struct ParsedGitLocator {
    pub(super) transport: GitTransport,
    pub(super) user: Option<String>,
    pub(super) host: String,
    pub(super) port: Option<CanonicalPort>,
    pub(super) repository_path: String,
}

impl ParsedGitLocator {
    pub(super) fn parse(locator: &str) -> Result<Self, IdentityError> {
        if locator.contains('?') || locator.contains('#') {
            return Err(IdentityError::QueryOrFragmentNotAllowed);
        }

        if let Some((scheme, remainder)) = locator.split_once("://") {
            return match scheme.to_ascii_lowercase().as_str() {
                "https" => Self::parse_url(GitTransport::Https, remainder),
                "ssh" => Self::parse_url(GitTransport::SshUrl, remainder),
                _ => Err(IdentityError::UnsupportedGitProtocol {
                    scheme: scheme.to_owned(),
                }),
            };
        }

        Self::parse_scp_like(locator)
    }

    fn parse_url(transport: GitTransport, remainder: &str) -> Result<Self, IdentityError> {
        let (authority, path) = remainder
            .split_once('/')
            .ok_or(IdentityError::MalformedGitLocator)?;
        if authority.is_empty() || path.is_empty() || path.starts_with('/') {
            return Err(IdentityError::MalformedGitLocator);
        }

        let (user, host_and_port) = match authority.rsplit_once('@') {
            Some((user_info, host)) => {
                if user_info.is_empty()
                    || user_info.contains(':')
                    || authority.matches('@').count() != 1
                {
                    return Err(IdentityError::CredentialsNotAllowed);
                }
                if transport == GitTransport::Https {
                    return Err(IdentityError::CredentialsNotAllowed);
                }
                validate_ssh_user(user_info)?;
                (Some(user_info.to_owned()), host)
            }
            None => {
                if transport == GitTransport::SshUrl {
                    return Err(IdentityError::MalformedGitLocator);
                }
                (None, authority)
            }
        };
        let (host, port) = parse_host_and_port(host_and_port)?;

        Ok(Self {
            transport,
            user,
            host,
            port,
            repository_path: validate_repository_path(path)?,
        })
    }

    fn parse_scp_like(locator: &str) -> Result<Self, IdentityError> {
        let (user_and_host, path) = locator
            .split_once(':')
            .ok_or(IdentityError::MalformedGitLocator)?;
        let (user, host) = user_and_host
            .split_once('@')
            .ok_or(IdentityError::MalformedGitLocator)?;
        if user.is_empty() || host.is_empty() || user.contains(':') || host.contains('@') {
            return Err(IdentityError::MalformedGitLocator);
        }
        validate_ssh_user(user)?;

        Ok(Self {
            transport: GitTransport::ScpLike,
            user: Some(user.to_owned()),
            host: validate_host(host)?,
            port: None,
            repository_path: validate_repository_path(path)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct CanonicalPort(String);

impl CanonicalPort {
    fn parse(value: &str) -> Result<Self, IdentityError> {
        if value.is_empty()
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || value.starts_with('0')
        {
            return Err(IdentityError::MalformedGitLocator);
        }
        let port = value
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or(IdentityError::MalformedGitLocator)?;
        Ok(Self(port.to_string()))
    }

    pub(super) fn get(&self) -> u16 {
        self.0.parse().expect("canonical port is a valid u16")
    }

    pub(super) fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

fn parse_host_and_port(value: &str) -> Result<(String, Option<CanonicalPort>), IdentityError> {
    if value.starts_with('[') || value.contains(']') || value.matches(':').count() > 1 {
        return Err(IdentityError::MalformedGitLocator);
    }
    let (host, port) = match value.rsplit_once(':') {
        Some((host, port)) => (host, Some(CanonicalPort::parse(port)?)),
        None => (value, None),
    };
    Ok((validate_host(host)?, port))
}

fn validate_host(value: &str) -> Result<String, IdentityError> {
    if value.is_empty()
        || value.starts_with('.')
        || value.ends_with('.')
        || value.split('.').any(|label| {
            label.is_empty()
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(IdentityError::MalformedGitLocator);
    }
    Ok(value.to_ascii_lowercase())
}

fn validate_ssh_user(value: &str) -> Result<(), IdentityError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(IdentityError::MalformedGitLocator);
    }
    Ok(())
}

pub(super) fn validate_repository_path(value: &str) -> Result<String, IdentityError> {
    if value.is_empty()
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains('\\')
        || value.contains('%')
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(IdentityError::MalformedRepositoryPath);
    }
    for component in value.split('/') {
        if component.is_empty()
            || matches!(component, "." | "..")
            || !component.bytes().all(is_repository_path_byte)
        {
            return Err(IdentityError::MalformedRepositoryPath);
        }
    }
    Ok(value.to_owned())
}

fn is_repository_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')
}

pub(super) fn is_github_owner(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 39
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

pub(super) fn is_github_repository(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && !matches!(value, "." | "..")
        && value.bytes().all(is_repository_path_byte)
}
