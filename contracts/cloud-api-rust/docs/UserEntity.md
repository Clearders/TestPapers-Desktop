# UserEntity

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**avatar_url** | Option<**String**> |  | [optional]
**created_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**display_name** | **String** |  |
**id** | **i32** |  |
**is_active** | **bool** |  |
**permissions** | **Vec<Permissions>** |  (enum: questions:read, questions:write, questions:delete, answers:read, papers:read, papers:write, users:manage) |
**public_id** | **String** |  |
**role** | [**models::UserRole**](UserRole.md) |  |
**updated_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**username** | **String** |  |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
