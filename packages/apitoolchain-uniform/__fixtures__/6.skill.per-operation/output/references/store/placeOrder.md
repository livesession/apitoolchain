<!-- Generated from the API reference. Do not edit by hand. -->

# `placeOrder` — Place order

`POST /store/orders`

Places an order for a pet.

## Headers

| Name | Type | Required | Description |
|---|---|---|---|
| `Idempotency-Key` | string | no | Retries with the same key place one order. |

## Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `pet_id` | string | yes | The pet to order. |
| `quantity` | integer | no | Default: `1`. Minimum: `1`. |
| `payment` | CardPayment or TransferPayment | yes | How the order is paid. |
| `note` | string or integer | no | A note for the shop. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 201 | `application/json` | Order | The order that was placed. |
| 409 | `application/json` | Error | The pet is not available. |

## Example request

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
