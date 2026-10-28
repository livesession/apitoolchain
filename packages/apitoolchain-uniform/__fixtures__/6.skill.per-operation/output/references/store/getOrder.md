<!-- Generated from the API reference. Do not edit by hand. -->

# `getOrder` — Retrieve order

`GET /store/orders/{id}`

Returns one order.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | integer | yes | The order's ID. |

## Query parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `expand` | array of string | no | Related objects to inline. Each one of: `pet`, `owner`. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | Order | The order. |
| 404 | `application/json` | Error | No order has this ID. |

### 200 — `application/json`

| Name | Type | Description |
|---|---|---|
| `id` | integer | The order's ID. |
| `pet_id` | string |  |
| `status` | string | One of: `placed`, `approved`, `delivered`. |
| `ship_date` | string or null |  |

### 404 — `application/json`

| Name | Type | Description |
|---|---|---|
| `code` | string | A machine-readable error code. |
| `message` | string | A human-readable message. |

## Example request

```shell
curl --request GET \
     --url https://api.example.com/v1/store/orders/0 \
     --header 'accept: application/json'
```
