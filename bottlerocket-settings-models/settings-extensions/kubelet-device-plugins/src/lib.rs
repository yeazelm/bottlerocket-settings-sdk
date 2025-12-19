//! Settings related to Kubelet Device Plugins
use bottlerocket_model_derive::model;
use bottlerocket_modeled_types::{NvidiaDevicePluginSettings, NvidiaDeviceSharingStrategy, NvidiaDevicePartitioningStrategy};
use bottlerocket_settings_sdk::{GenerateResult, SettingsModel};
use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum KubeletDevicePluginsError {
    #[snafu(display("MPS and MIG cannot be enabled simultaneously. NVIDIA does not support MPS on MIG-partitioned devices."))]
    MpsMigConflict,
}

#[model(impl_default = true)]
pub struct KubeletDevicePluginsV1 {
    nvidia: NvidiaDevicePluginSettings,
}

type Result<T> = std::result::Result<T, KubeletDevicePluginsError>;

impl SettingsModel for KubeletDevicePluginsV1 {
    type PartialKind = Self;
    type ErrorKind = KubeletDevicePluginsError;

    fn get_version() -> &'static str {
        "v1"
    }

    fn set(_current_value: Option<Self>, _target: Self) -> Result<()> {
        Ok(())
    }

    fn generate(
        existing_partial: Option<Self::PartialKind>,
        _dependent_settings: Option<serde_json::Value>,
    ) -> Result<GenerateResult<Self::PartialKind, Self>> {
        Ok(GenerateResult::Complete(
            existing_partial.unwrap_or_default(),
        ))
    }

    fn validate(value: Self, _validated_settings: Option<serde_json::Value>) -> Result<()> {
        // Validate MPS and MIG are not both enabled
        if let Some(ref nvidia) = value.nvidia {
            let is_mps = matches!(
                nvidia.device_sharing_strategy,
                Some(NvidiaDeviceSharingStrategy::Mps)
            );
            let is_mig = matches!(
                nvidia.device_partitioning_strategy,
                Some(NvidiaDevicePartitioningStrategy::MIG)
            );
            if is_mps && is_mig {
                return Err(KubeletDevicePluginsError::MpsMigConflict);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bottlerocket_modeled_types::{
        MigProfile, NvidiaDeviceIdStrategy, NvidiaDeviceListStrategy,
        NvidiaDeviceListStrategyValues, NvidiaDevicePartitioningStrategy,
        NvidiaDeviceSharingStrategy, NvidiaGpuModel, NvidiaMigSettings, NvidiaTimeSlicingSettings,
        NvidiaMpsSettings,
    };
    use bounded_integer::BoundedI32;
    use std::collections::HashMap;

    #[test]
    fn test_generate_kubelet_device_plugins() {
        let generated = KubeletDevicePluginsV1::generate(None, None).unwrap();
        assert_eq!(
            generated,
            GenerateResult::Complete(KubeletDevicePluginsV1 { nvidia: None })
        );
    }

    #[test]
    fn test_mps_mig_mutual_exclusion() {
        let settings = KubeletDevicePluginsV1 {
            nvidia: Some(NvidiaDevicePluginSettings {
                device_sharing_strategy: Some(NvidiaDeviceSharingStrategy::Mps),
                device_partitioning_strategy: Some(NvidiaDevicePartitioningStrategy::MIG),
                ..Default::default()
            }),
        };
        let result = KubeletDevicePluginsV1::validate(settings, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_mps_alone_valid() {
        let settings = KubeletDevicePluginsV1 {
            nvidia: Some(NvidiaDevicePluginSettings {
                device_sharing_strategy: Some(NvidiaDeviceSharingStrategy::Mps),
                device_partitioning_strategy: Some(NvidiaDevicePartitioningStrategy::None),
                ..Default::default()
            }),
        };
        let result = KubeletDevicePluginsV1::validate(settings, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mig_alone_valid() {
        let settings = KubeletDevicePluginsV1 {
            nvidia: Some(NvidiaDevicePluginSettings {
                device_sharing_strategy: Some(NvidiaDeviceSharingStrategy::None),
                device_partitioning_strategy: Some(NvidiaDevicePartitioningStrategy::MIG),
                ..Default::default()
            }),
        };
        let result = KubeletDevicePluginsV1::validate(settings, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_serde_kubelet_device_plugins_with_mps() {
        let test_json = r#"{"nvidia":{"pass-device-specs":true,"device-id-strategy":"index","device-list-strategy":"volume-mounts","device-sharing-strategy":"mps","mps":{"replicas":4},"device-partitioning-strategy":"none"}}"#;

        let device_plugins: KubeletDevicePluginsV1 = serde_json::from_str(test_json).unwrap();
        assert_eq!(
            device_plugins.nvidia.as_ref().unwrap().device_sharing_strategy,
            Some(NvidiaDeviceSharingStrategy::Mps)
        );
    }

    #[test]
    fn test_serde_kubelet_device_plugins_vec() {
        let test_json = r#"{"nvidia":{"pass-device-specs":true,"device-id-strategy":"index","device-list-strategy":["volume-mounts","envvar"],"device-sharing-strategy":"time-slicing","time-slicing":{"replicas":2,"rename-by-default":true,"fail-requests-greater-than-one":true},"mps":{},"device-partitioning-strategy":"mig","mig":{"profile":{"a100.40gb":"1g.5gb"}}}}"#;

        let device_plugins: KubeletDevicePluginsV1 = serde_json::from_str(test_json).unwrap();
        assert_eq!(
            device_plugins,
            KubeletDevicePluginsV1 {
                nvidia: Some(NvidiaDevicePluginSettings {
                    pass_device_specs: Some(true),
                    device_id_strategy: Some(NvidiaDeviceIdStrategy::Index),
                    device_list_strategy: Some(NvidiaDeviceListStrategy::Vector(vec![
                        NvidiaDeviceListStrategyValues::VolumeMounts,
                        NvidiaDeviceListStrategyValues::Envvar,
                    ])),
                    device_sharing_strategy: Some(NvidiaDeviceSharingStrategy::TimeSlicing),
                    time_slicing: Some(NvidiaTimeSlicingSettings {
                        replicas: Some(BoundedI32::new(2).unwrap()),
                        rename_by_default: Some(true),
                        fail_requests_greater_than_one: Some(true),
                    }),
                    mps: Some(NvidiaMpsSettings::default()),
                    device_partitioning_strategy: Some(NvidiaDevicePartitioningStrategy::MIG),
                    mig: Some(NvidiaMigSettings {
                        profile: Some(HashMap::from([(
                            NvidiaGpuModel::try_from("a100.40gb").unwrap(),
                            MigProfile::try_from("1g.5gb").unwrap()
                        )]))
                    }),
                })
            }
        );
    }

    #[test]
    fn test_serde_kubelet_device_plugins_scalar() {
        let test_json = r#"{"nvidia":{"pass-device-specs":true,"device-id-strategy":"index","device-list-strategy":"volume-mounts","device-sharing-strategy":"time-slicing","time-slicing":{"replicas":2,"rename-by-default":true,"fail-requests-greater-than-one":true},"mps":{},"device-partitioning-strategy":"mig","mig":{"profile":{"a100.40gb":"1g.5gb"}}}}"#;

        let device_plugins: KubeletDevicePluginsV1 = serde_json::from_str(test_json).unwrap();
        assert_eq!(
            device_plugins,
            KubeletDevicePluginsV1 {
                nvidia: Some(NvidiaDevicePluginSettings {
                    pass_device_specs: Some(true),
                    device_id_strategy: Some(NvidiaDeviceIdStrategy::Index),
                    device_list_strategy: Some(NvidiaDeviceListStrategy::Scalar(
                        NvidiaDeviceListStrategyValues::VolumeMounts
                    )),
                    device_sharing_strategy: Some(NvidiaDeviceSharingStrategy::TimeSlicing),
                    time_slicing: Some(NvidiaTimeSlicingSettings {
                        replicas: Some(BoundedI32::new(2).unwrap()),
                        rename_by_default: Some(true),
                        fail_requests_greater_than_one: Some(true),
                    }),
                    mps: Some(NvidiaMpsSettings::default()),
                    device_partitioning_strategy: Some(NvidiaDevicePartitioningStrategy::MIG),
                    mig: Some(NvidiaMigSettings {
                        profile: Some(HashMap::from([(
                            NvidiaGpuModel::try_from("a100.40gb").unwrap(),
                            MigProfile::try_from("1g.5gb").unwrap()
                        )]))
                    }),
                })
            }
        );
    }
}
