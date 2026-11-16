-- Lua に u32 型はないため、Rust へ渡す前の値の範囲を確認する。
local M = {}
function M.is_u32(value)
    return type(value) == "number" and value >= 0 and value <= 4294967295
        and value == math.floor(value)
end
function M.validate_nutrition(value)
    assert(type(value) == "table", "nutrition must be a table")
    for _, key in ipairs({ "protein", "carbohydrate", "vitamin" }) do
        assert(M.is_u32(value[key]), "nutrition." .. key .. " must be a u32 integer")
    end
    for key in pairs(value) do
        assert(key == "protein" or key == "carbohydrate" or key == "vitamin", "unknown nutrition key")
    end
end
return M
