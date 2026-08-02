# \UsersApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_user**](UsersApi.md#create_user) | **POST** /api/v1/users | Create User
[**delete_user**](UsersApi.md#delete_user) | **DELETE** /api/v1/users/{user_public_id} | Delete User
[**list_users**](UsersApi.md#list_users) | **GET** /api/v1/users | List Users
[**update_user**](UsersApi.md#update_user) | **PATCH** /api/v1/users/{user_public_id} | Update User



## create_user

> models::EnvelopeUserEntity create_user(user_create)
Create User

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**user_create** | [**UserCreate**](UserCreate.md) |  | [required] |

### Return type

[**models::EnvelopeUserEntity**](Envelope_UserEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_user

> delete_user(user_public_id)
Delete User

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**user_public_id** | **String** |  | [required] |

### Return type

 (empty response body)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_users

> models::EnvelopeListUserEntity list_users()
List Users

### Parameters

This endpoint does not need any parameter.

### Return type

[**models::EnvelopeListUserEntity**](Envelope_list_UserEntity__.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## update_user

> models::EnvelopeUserEntity update_user(user_public_id, user_update)
Update User

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**user_public_id** | **String** |  | [required] |
**user_update** | [**UserUpdate**](UserUpdate.md) |  | [required] |

### Return type

[**models::EnvelopeUserEntity**](Envelope_UserEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)
