<!-- Generated from openapi.yaml (store) -- regenerate, do not edit -- > -->

# store

## `placeOrder` — Place order

`POST /store/orders`

Places an order for a pet.

### Headers

| Name | Type | Required | Description |
|---|---|---|---|
| `Idempotency-Key` | string | no | Retries with the same key place one order. |

### Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `pet_id` | string | yes | The pet to order. |
| `quantity` | integer | no | Default: `1`. Minimum: `1`. |
| `payment` | CardPayment or TransferPayment | yes | How the order is paid. |
| `note` | string or integer | no | A note for the shop. |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 201 | `application/json` | Order | The order that was placed. |
| 409 | `application/json` | Error | The pet is not available. |

### Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/store/orders \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '
{
  "pet_id": "string",
  "quantity": 1,
  "payment": {
    "type": "card",
    "token": "string"
  },
  "note": "string"
}
'
```

## `getOrder` — Retrieve order

`GET /store/orders/{id}`

Returns one order.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | integer | yes | The order's ID. |

### Query parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `expand` | array of string | no | Related objects to inline. Each one of: `pet`, `owner`. |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | Order | The order. |
| 404 | `application/json` | Error | No order has this ID. |

### Example request

```shell
curl --request GET \
     --url https://api.example.com/v1/store/orders/0 \
     --header 'accept: application/json'
```

## `replaceOrder` — Replace order

`PUT /store/orders/{id}`

Replaces an order; the body's `id` must match the path.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | integer | yes | The order's ID. |

### Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | integer | yes | The order's ID. |
| `pet_id` | string | yes |  |
| `status` | string | yes | One of: `placed`, `approved`, `delivered`. |
| `ship_date` | string or null | no |  |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | Order | The replaced order. |

### Example request

```shell
curl --request PUT \
     --url https://api.example.com/v1/store/orders/0 \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '
{
  "id": 0,
  "pet_id": "string",
  "status": "placed",
  "ship_date": "2019-08-24T14:15:22Z"
}
'
```

## `cancelOrder` — Cancel order

`DELETE /store/orders/{id}`

Cancels an order that has not shipped.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | integer | yes | The order's ID. |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The order was cancelled. |

### Example request

```shell
curl --request DELETE \
     --url https://api.example.com/v1/store/orders/0
```
