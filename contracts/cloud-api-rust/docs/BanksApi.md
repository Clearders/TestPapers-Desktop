# \BanksApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**add_items**](BanksApi.md#add_items) | **POST** /api/v1/banks/{bank_public_id}/items | Add Items
[**create_bank_route**](BanksApi.md#create_bank_route) | **POST** /api/v1/banks | Create Bank Route
[**create_member**](BanksApi.md#create_member) | **POST** /api/v1/banks/{bank_public_id}/members | Create Member
[**delete_bank_route**](BanksApi.md#delete_bank_route) | **DELETE** /api/v1/banks/{bank_public_id} | Delete Bank Route
[**fork**](BanksApi.md#fork) | **POST** /api/v1/banks/{bank_public_id}/fork | Fork
[**get_bank**](BanksApi.md#get_bank) | **GET** /api/v1/banks/{bank_public_id} | Get Bank
[**get_bank_questions**](BanksApi.md#get_bank_questions) | **GET** /api/v1/banks/{bank_public_id}/questions | Get Bank Questions
[**get_version**](BanksApi.md#get_version) | **GET** /api/v1/banks/{bank_public_id}/versions/{version} | Get Version
[**list_banks**](BanksApi.md#list_banks) | **GET** /api/v1/banks | List Banks
[**patch_bank**](BanksApi.md#patch_bank) | **PATCH** /api/v1/banks/{bank_public_id} | Patch Bank
[**patch_member**](BanksApi.md#patch_member) | **PATCH** /api/v1/banks/{bank_public_id}/members/{user_public_id} | Patch Member
[**patch_subscription**](BanksApi.md#patch_subscription) | **PATCH** /api/v1/banks/{bank_public_id}/subscribe | Patch Subscription
[**publish**](BanksApi.md#publish) | **POST** /api/v1/banks/{bank_public_id}/publish | Publish
[**remove_item**](BanksApi.md#remove_item) | **DELETE** /api/v1/banks/{bank_public_id}/items/{question_public_id} | Remove Item
[**remove_member**](BanksApi.md#remove_member) | **DELETE** /api/v1/banks/{bank_public_id}/members/{user_public_id} | Remove Member
[**subscribe**](BanksApi.md#subscribe) | **POST** /api/v1/banks/{bank_public_id}/subscribe | Subscribe
[**unsubscribe**](BanksApi.md#unsubscribe) | **DELETE** /api/v1/banks/{bank_public_id}/subscribe | Unsubscribe
[**versions**](BanksApi.md#versions) | **GET** /api/v1/banks/{bank_public_id}/versions | Versions
[**withdraw**](BanksApi.md#withdraw) | **POST** /api/v1/banks/{bank_public_id}/withdraw | Withdraw



## add_items

> models::EnvelopeQuestionBankEntity add_items(bank_public_id, bank_item_add)
Add Items

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |
**bank_item_add** | [**BankItemAdd**](BankItemAdd.md) |  | [required] |

### Return type

[**models::EnvelopeQuestionBankEntity**](Envelope_QuestionBankEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## create_bank_route

> models::EnvelopeQuestionBankEntity create_bank_route(bank_create)
Create Bank Route

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_create** | [**BankCreate**](BankCreate.md) |  | [required] |

### Return type

[**models::EnvelopeQuestionBankEntity**](Envelope_QuestionBankEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## create_member

> models::EnvelopeQuestionBankEntity create_member(bank_public_id, bank_member_create)
Create Member

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |
**bank_member_create** | [**BankMemberCreate**](BankMemberCreate.md) |  | [required] |

### Return type

[**models::EnvelopeQuestionBankEntity**](Envelope_QuestionBankEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_bank_route

> delete_bank_route(bank_public_id)
Delete Bank Route

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |

### Return type

 (empty response body)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## fork

> models::EnvelopeQuestionBankEntity fork(bank_public_id, bank_fork_request)
Fork

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |
**bank_fork_request** | [**BankForkRequest**](BankForkRequest.md) |  | [required] |

### Return type

[**models::EnvelopeQuestionBankEntity**](Envelope_QuestionBankEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_bank

> models::EnvelopeQuestionBankEntity get_bank(bank_public_id)
Get Bank

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeQuestionBankEntity**](Envelope_QuestionBankEntity_.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_bank_questions

> models::EnvelopeListQuestionEntity get_bank_questions(bank_public_id)
Get Bank Questions

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeListQuestionEntity**](Envelope_list_QuestionEntity__.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_version

> models::EnvelopeBankPublicationEntity get_version(bank_public_id, version)
Get Version

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |
**version** | **i32** |  | [required] |

### Return type

[**models::EnvelopeBankPublicationEntity**](Envelope_BankPublicationEntity_.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_banks

> models::EnvelopeListQuestionBankSummary list_banks(q, visibility, scope)
List Banks

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**q** | Option<**String**> |  |  |
**visibility** | Option<[**models::BankVisibility**](Models__BankVisibility.md)> |  |  |
**scope** | Option<[**models::BankListScope**](Models__BankListScope.md)> |  |  |[default to visible]

### Return type

[**models::EnvelopeListQuestionBankSummary**](Envelope_list_QuestionBankSummary__.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## patch_bank

> models::EnvelopeQuestionBankEntity patch_bank(bank_public_id, bank_update)
Patch Bank

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |
**bank_update** | [**BankUpdate**](BankUpdate.md) |  | [required] |

### Return type

[**models::EnvelopeQuestionBankEntity**](Envelope_QuestionBankEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## patch_member

> models::EnvelopeQuestionBankEntity patch_member(bank_public_id, user_public_id, bank_member_update)
Patch Member

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |
**user_public_id** | **String** |  | [required] |
**bank_member_update** | [**BankMemberUpdate**](BankMemberUpdate.md) |  | [required] |

### Return type

[**models::EnvelopeQuestionBankEntity**](Envelope_QuestionBankEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## patch_subscription

> models::EnvelopeBankSubscriptionEntity patch_subscription(bank_public_id, bank_subscription_update)
Patch Subscription

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |
**bank_subscription_update** | [**BankSubscriptionUpdate**](BankSubscriptionUpdate.md) |  | [required] |

### Return type

[**models::EnvelopeBankSubscriptionEntity**](Envelope_BankSubscriptionEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## publish

> models::EnvelopeQuestionBankEntity publish(bank_public_id)
Publish

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeQuestionBankEntity**](Envelope_QuestionBankEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## remove_item

> models::EnvelopeQuestionBankEntity remove_item(bank_public_id, question_public_id)
Remove Item

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |
**question_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeQuestionBankEntity**](Envelope_QuestionBankEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## remove_member

> models::EnvelopeQuestionBankEntity remove_member(bank_public_id, user_public_id)
Remove Member

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |
**user_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeQuestionBankEntity**](Envelope_QuestionBankEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## subscribe

> models::EnvelopeBankSubscriptionEntity subscribe(bank_public_id)
Subscribe

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeBankSubscriptionEntity**](Envelope_BankSubscriptionEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## unsubscribe

> unsubscribe(bank_public_id)
Unsubscribe

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |

### Return type

 (empty response body)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## versions

> models::EnvelopeListBankVersionSummary versions(bank_public_id)
Versions

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeListBankVersionSummary**](Envelope_list_BankVersionSummary__.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## withdraw

> models::EnvelopeQuestionBankEntity withdraw(bank_public_id)
Withdraw

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeQuestionBankEntity**](Envelope_QuestionBankEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)
