# SyncConflictResolutionRecord

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**accepted_content_hash** | **String** |  |
**accepted_version** | **i32** |  |
**action** | [**models::SyncResolutionAction**](SyncResolutionAction.md) |  |
**actor_device_id** | **String** |  |
**conflict_id** | **String** |  |
**new_entity_id** | Option<**String**> |  | [optional]
**operation_id** | **String** |  |
**protocol_version** | **ProtocolVersion** |  (enum: 1) |
**resolution_id** | **String** |  |
**resolved_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**result** | [**models::SyncConflictSnapshot**](SyncConflictSnapshot.md) |  |
**undoes_resolution_id** | Option<**String**> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
