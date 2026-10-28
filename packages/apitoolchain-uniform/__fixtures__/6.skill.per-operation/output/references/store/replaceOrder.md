<!-- Generated from the API reference. Do not edit by hand. -->

# `replaceOrder` — Replace order

`PUT /store/orders/{id}`

Replaces an order; the body's `id` must match the path.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | integer | yes | The order's ID. |

## Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | integer | yes | The order's ID. |
| `pet_id` | string | yes |  |
| `status` | string | yes | One of: `placed`, `approved`, `delivered`. |
| `ship_date` | string or null | no |  |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | Order | The replaced order. |

### 200 — `application/json`

| Name | Type | Description |
|---|---|---|
| `id` | integer | The order's ID. |
| `pet_id` | string |  |
| `status` | string | One of: `placed`, `approved`, `delivered`. |
| `ship_date` | string or null |  |

## Example request

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
