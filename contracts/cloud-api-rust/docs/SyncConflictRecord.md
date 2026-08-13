# SyncConflictRecord

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**base** | Option<[**models::SyncConflictSnapshot**](SyncConflictSnapshot.md)> |  |
**cloud** | [**models::SyncConflictSnapshot**](SyncConflictSnapshot.md) |  |
**conflict_id** | **String** |  |
**detected_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**entity_id** | **String** |  |
**entity_type** | [**models::SyncEntityType**](SyncEntityType.md) |  |
**local** | [**models::SyncConflictSnapshot**](SyncConflictSnapshot.md) |  |
**origin** | **Origin** |  (enum: personalSync) |
**protocol_version** | **ProtocolVersion** |  (enum: 1) |
**reason** | [**models::SyncConflictReason**](SyncConflictReason.md) |  |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
