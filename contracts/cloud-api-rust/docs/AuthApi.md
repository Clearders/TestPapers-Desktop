# \AuthApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**change_password**](AuthApi.md#change_password) | **PUT** /api/v1/auth/password | Change Password
[**delete_account**](AuthApi.md#delete_account) | **DELETE** /api/v1/auth/account | Delete Account
[**delete_device**](AuthApi.md#delete_device) | **DELETE** /api/v1/auth/devices/{device_id} | Delete Device
[**get_devices**](AuthApi.md#get_devices) | **GET** /api/v1/auth/devices | Get Devices
[**get_me**](AuthApi.md#get_me) | **GET** /api/v1/auth/me | Get Me
[**login**](AuthApi.md#login) | **POST** /api/v1/auth/login | Login
[**logout**](AuthApi.md#logout) | **POST** /api/v1/auth/logout | Logout
[**native_login**](AuthApi.md#native_login) | **POST** /api/v1/auth/token | Native Login
[**native_refresh**](AuthApi.md#native_refresh) | **POST** /api/v1/auth/token/refresh | Native Refresh
[**refresh_session**](AuthApi.md#refresh_session) | **POST** /api/v1/auth/refresh | Refresh Session
[**register**](AuthApi.md#register) | **POST** /api/v1/auth/register | Register
[**update_profile**](AuthApi.md#update_profile) | **PATCH** /api/v1/auth/profile | Update Profile
[**upload_avatar**](AuthApi.md#upload_avatar) | **POST** /api/v1/auth/avatar | Upload Avatar



## change_password

> serde_json::Value change_password(password_change)
Change Password

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**password_change** | [**PasswordChange**](PasswordChange.md) |  | [required] |

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_account

> delete_account()
Delete Account

### Parameters

This endpoint does not need any parameter.

### Return type

 (empty response body)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_device

> delete_device(device_id)
Delete Device

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**device_id** | **String** |  | [required] |

### Return type

 (empty response body)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_devices

> models::EnvelopeListDeviceSessionEntity get_devices()
Get Devices

### Parameters

This endpoint does not need any parameter.

### Return type

[**models::EnvelopeListDeviceSessionEntity**](Envelope_list_DeviceSessionEntity__.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_me

> models::EnvelopeUserEntity get_me()
Get Me

### Parameters

This endpoint does not need any parameter.

### Return type

[**models::EnvelopeUserEntity**](Envelope_UserEntity_.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## login

> models::EnvelopeAuthSession login(login_request)
Login

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**login_request** | [**LoginRequest**](LoginRequest.md) |  | [required] |

### Return type

[**models::EnvelopeAuthSession**](Envelope_AuthSession_.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## logout

> logout()
Logout

### Parameters

This endpoint does not need any parameter.

### Return type

 (empty response body)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## native_login

> models::EnvelopeTokenPair native_login(native_login_request)
Native Login

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**native_login_request** | [**NativeLoginRequest**](NativeLoginRequest.md) |  | [required] |

### Return type

[**models::EnvelopeTokenPair**](Envelope_TokenPair_.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## native_refresh

> models::EnvelopeTokenPair native_refresh(refresh_token_request)
Native Refresh

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**refresh_token_request** | [**RefreshTokenRequest**](RefreshTokenRequest.md) |  | [required] |

### Return type

[**models::EnvelopeTokenPair**](Envelope_TokenPair_.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## refresh_session

> models::EnvelopeAuthSession refresh_session()
Refresh Session

### Parameters

This endpoint does not need any parameter.

### Return type

[**models::EnvelopeAuthSession**](Envelope_AuthSession_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## register

> models::EnvelopeAuthSession register(register_request)
Register

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**register_request** | [**RegisterRequest**](RegisterRequest.md) |  | [required] |

### Return type

[**models::EnvelopeAuthSession**](Envelope_AuthSession_.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## update_profile

> models::EnvelopeUserEntity update_profile(profile_update)
Update Profile

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**profile_update** | [**ProfileUpdate**](ProfileUpdate.md) |  | [required] |

### Return type

[**models::EnvelopeUserEntity**](Envelope_UserEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## upload_avatar

> models::EnvelopeImageUploadResponse upload_avatar(image_upload_payload)
Upload Avatar

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**image_upload_payload** | [**ImageUploadPayload**](ImageUploadPayload.md) |  | [required] |

### Return type

[**models::EnvelopeImageUploadResponse**](Envelope_ImageUploadResponse_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)
