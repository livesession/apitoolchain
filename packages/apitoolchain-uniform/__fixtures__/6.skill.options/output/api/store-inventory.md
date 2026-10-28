<!-- Generated from openapi.yaml (Store / Inventory) -- regenerate, do not edit -- > -->

# Store / Inventory

## `store-inventory` — Store inventory

`GET /store/inventory`

Returns pet counts by status.

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | — | Counts by status. |

### Example request

```shell
curl --request GET \
     --url https://api.example.com/v1/store/inventory \
     --header 'accept: application/json'
```
