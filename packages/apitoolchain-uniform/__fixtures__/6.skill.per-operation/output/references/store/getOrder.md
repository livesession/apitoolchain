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

## Example request

```shell
curl --request GET \
     --url https://api.example.com/v1/store/orders/0 \
     --header 'accept: application/json'
```
