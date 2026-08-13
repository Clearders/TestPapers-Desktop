# SyncEntityVersionRecord

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**content_hash** | **String** |  |
**created_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**device_id** | **String** |  |
**entity_id** | **String** |  |
**entity_type** | [**models::SyncEntityType**](SyncEntityType.md) |  |
**mutation_kind** | [**models::SyncMutationKind**](SyncMutationKind.md) |  |
**operation_id** | **String** |  |
**payload** | Option<**std::collections::HashMap<String, serde_json::Value>**> |  |
**schema_version** | **i32** |  |
**tombstone** | **bool** |  |
**version** | **i32** |  |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
