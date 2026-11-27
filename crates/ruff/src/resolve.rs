use std::path::Path;

use anyhow::{Result, bail};
use log::debug;

use ruff_workspace::configuration::Configuration;
use ruff_workspace::pyproject::{self, find_fallback_target_version};
use ruff_workspace::resolver::{
    ConfigurationOrigin, ConfigurationTransformer, PyprojectDiscoveryStrategy, ResolvedSettings,
    RootSettings, resolve_root_settings,
};

use ruff_python_ast as ast;

use crate::args::ConfigArguments;

/// Resolve the relevant settings strategy and defaults for the current
/// invocation.
pub fn resolve(
    config_arguments: &ConfigArguments,
    stdin_filename: Option<&Path>,
) -> Result<RootSettings> {
    let Ok(cwd) = std::env::current_dir() else {
        bail!("Working directory does not exist")
    };

    if config_arguments.isolated {
        debug!("Isolated mode, not reading any pyproject.toml");
        let configuration = config_arguments.transform(Configuration::default());
        return Ok(RootSettings {
            strategy: PyprojectDiscoveryStrategy::Fixed,
            resolved: ResolvedSettings::new(configuration.into_settings(&cwd)?),
        });
    }

    if let Some(pyproject) = config_arguments.config_file() {
        let resolved = resolve_root_settings(
            pyproject,
            config_arguments,
            ConfigurationOrigin::UserSpecified,
        )?;
        debug!(
            "Using user-specified configuration file at: {}",
            pyproject.display()
        );
        return Ok(RootSettings {
            strategy: PyprojectDiscoveryStrategy::Fixed,
            resolved,
        });
    }

    if let Some(pyproject) = pyproject::find_settings_toml(stdin_filename.unwrap_or(&cwd))? {
        debug!(
            "Using configuration file (via parent) at: {}",
            pyproject.display()
        );
        let resolved =
            resolve_root_settings(&pyproject, config_arguments, ConfigurationOrigin::Ancestor)?;
        return Ok(RootSettings {
            strategy: PyprojectDiscoveryStrategy::Hierarchical,
            resolved,
        });
    }

    if let Some(pyproject) = pyproject::find_user_settings_toml() {
        debug!(
            "Using configuration file (via cwd) at: {}",
            pyproject.display()
        );
        let resolved = resolve_root_settings(
            &pyproject,
            config_arguments,
            ConfigurationOrigin::UserSettings {
                fallback_directory: &cwd,
            },
        )?;
        return Ok(RootSettings {
            strategy: PyprojectDiscoveryStrategy::Hierarchical,
            resolved,
        });
    }

    debug!("Using Ruff default settings");
    let mut configuration = config_arguments.transform(Configuration::default());
    if configuration.target_version.is_none() {
        let fallback = find_fallback_target_version(stdin_filename.unwrap_or(&cwd));
        if let Some(version) = fallback {
            debug!("Derived `target-version` from found `requires-python`: {version:?}");
        }
        configuration.target_version = fallback.map(ast::PythonVersion::from);
    }
    Ok(RootSettings {
        strategy: PyprojectDiscoveryStrategy::Hierarchical,
        resolved: ResolvedSettings::new(configuration.into_settings(&cwd)?),
    })
}
