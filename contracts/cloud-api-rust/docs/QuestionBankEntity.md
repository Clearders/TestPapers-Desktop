# QuestionBankEntity

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**access_role** | [**models::BankAccessRole**](BankAccessRole.md) |  |
**created_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**description** | **String** |  |
**has_update** | Option<**bool**> |  | [optional][default to false]
**id** | **i32** |  |
**is_subscribed** | Option<**bool**> |  | [optional][default to false]
**item_count** | Option<**i32**> |  | [optional][default to 0]
**member_count** | Option<**i32**> |  | [optional][default to 0]
**members** | Option<[**Vec<models::BankMemberEntity>**](BankMemberEntity.md)> |  | [optional]
**name** | **String** |  |
**owner** | Option<[**models::BankUserRef**](BankUserRef.md)> |  | [optional]
**public_id** | **String** |  |
**subscribed_version** | Option<**i32**> |  | [optional]
**subscriber_count** | Option<**i32**> |  | [optional][default to 0]
**updated_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**version** | Option<**i32**> |  | [optional]
**visibility** | [**models::BankVisibility**](BankVisibility.md) |  |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
