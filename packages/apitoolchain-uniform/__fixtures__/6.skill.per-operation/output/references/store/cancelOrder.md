<!-- Generated from the API reference. Do not edit by hand. -->

# `cancelOrder` — Cancel order

`DELETE /store/orders/{id}`

Cancels an order that has not shipped.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | integer | yes | The order's ID. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The order was cancelled. |

## Example request

```shell
curl --request DELETE \
     --url https://api.example.com/v1/store/orders/0
```
