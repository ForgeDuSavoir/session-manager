//! Platform-independent Session Manager core.
//!
//! This crate deliberately contains no concrete desktop integration. External effects
//! are expressed as ports, while the domain remains independent of I/O.

pub mod adapter_contract;
pub mod application;
pub mod configuration;
pub mod domain;
pub mod interface;
pub mod storage_port;

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod architecture_tests {
    const DOMAIN: &str = include_str!("domain.rs");
    const APPLICATION: &str = include_str!("application.rs");
    const CONFIGURATION: &str = include_str!("configuration.rs");

    #[test]
    fn domain_and_application_do_not_name_concrete_platforms() {
        let forbidden = [
            "hyprland",
            "sway",
            "wayland",
            "x11",
            "dbus",
            "systemd",
            "compositor",
            "display",
        ];
        let source = format!("{DOMAIN}\n{APPLICATION}").to_ascii_lowercase();

        for name in forbidden {
            assert!(
                !source.contains(name),
                "core module must not mention concrete platform term: {name}"
            );
        }
    }

    #[test]
    fn application_depends_only_on_domain_and_ports() {
        let forbidden_imports = ["crate::interface", "crate::adapter_contract"];
        for import in forbidden_imports {
            assert!(
                !APPLICATION.contains(import),
                "application must point inward and not import {import}"
            );
        }
    }

    #[test]
    fn configuration_validation_has_no_runtime_or_platform_dependencies() {
        let forbidden = [
            "crate::adapter_contract",
            "crate::application",
            "crate::interface",
            "crate::storage_port",
            "std::env",
            "std::net",
            "std::os::unix::net",
            "Command",
        ];
        for dependency in forbidden {
            assert!(
                !CONFIGURATION.contains(dependency),
                "configuration validation must remain offline: {dependency}"
            );
        }
    }
}
