<!-- Generated from the API reference. Do not edit by hand. -->

# `shipOrder` — Ship order

`POST /store/orders/{id}/ship`

Ships an order with carrier No. 1 unless told otherwise. A tracking
code follows by email.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | integer | yes | The order's ID. |

## Request body

Content type: `application/json`. Optional.

| Name | Type | Required | Description |
|---|---|---|---|
| `carrier` | string | yes | The carrier to use. |
| `tracking` | boolean | no | Whether to send a tracking code. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 202 | `application/json` | Order | The order is on its way. |

## Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/store/orders/0/ship \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '
{
  "carrier": "string",
  "tracking": true
}
'
```
