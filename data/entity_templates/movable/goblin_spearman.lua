-- 判断と調整はLua、位置・拾得・装備・警報の整合性は共通Rust APIで検査する。
local function distance(a, b)
    return math.max(math.abs(a.x-b.x), math.abs(a.y-b.y))
end
local directions = {{-1,0},{0,1},{0,-1},{1,0},{-1,-1},{1,-1},{-1,1},{1,1}}
local function retreat(self, ctx, target)
    local best, best_distance = nil, distance(self.position, target.position)
    for _, d in ipairs(directions) do
        local p = {x=self.position.x+d[1],y=self.position.y+d[2],z=self.position.z}
        local n = distance(p, target.position)
        if n > best_distance and ctx:can_step(self.id,p) then best,best_distance=p,n end
    end
    if best then return {action={type="move",position=best}} end
end
local function wander(self, ctx, turn)
    for i=0,7 do
        local d=directions[(turn+self.id+i)%8+1]
        local p={x=self.position.x+d[1],y=self.position.y+d[2],z=self.position.z}
        if ctx:can_step(self.id,p) then return {action={type="move",position=p}} end
    end
    return {action={type="wait"}}
end
return {
    schema_version = 2,
    id = "monster.goblin_spearman",
    category = "movable",
    common = {
        name = "ゴブリンの槍持ち",
        description = "長い自作槍で二歩先を突くゴブリン。盾役の後ろや狭い通路で危険。",
        appearance = {
            glyph = "g",
            fg = { 153, 160, 107 },
            bg = { 0, 0, 0 },
            layer = 30,
        },
        collision = {
            movement = true,
            sight = false,
        },
        durability = false,
        tags = { "goblin" },
    },
    initial_state = {},
    movable = {
        tier = 2,
        experience_reward = 45,
        dominant_hand = "right",
        sanity = 100,
        psykick = {},
        status = {},
        combat = { base_av = 0, dv_bonus = 0, accuracy_bonus = -1 },
        role = "monster",
        controller = "lua",
        faction = "goblin",
        hostile_factions = { "survivor", "ghoul" },
        lua_alerts = true,
        hp = 20,
        body = { "head", "body", "left_arm", "right_arm", "left_leg", "right_leg" },
        attributes = {
            kinryoku = 7,
            subayasa = 8,
            kiyousa = 8,
            seishinryoku = 4,
        },
        skills = {
            melee_weapon = 2,
            bow = 0,
        },
        inventory = {
            max_length_mm = 1500,
            max_volume_ml = 20000,
            max_mass_g = 30000,
        },
        senses = {
            sight_range = 10,
        },
        attack = {
            damage = { dice = 1, sides = 3, bonus = 0 },
            base_pv = 3, strength_cap = 2,
            range = 1,
        },
        loadout = {{definition_id="item.spear",quantity=1,equipped=true,grip="both"}},
        needs = {
            hydration = {
                max = 100,
                initial = 80,
            },
        },
    },
    hooks = {
        on_turn = function(self, ctx, event)
            local target=ctx:nearest_hostile(self.id,self.config.movable.senses.sight_range)
            if not target and self.alert_position and event.turn <= self.alert_until then
                local p=ctx:step_towards_position(self.id,self.alert_position)
                if p then return {action={type="move",position=p}} end
            end
            if not target then return wander(self,ctx,event.turn) end
            if ctx:can_attack(self.id,target.id) then return {action={type="attack",target=target.id}} end
            local p=ctx:step_towards(self.id,target.id)
            if p then return {action={type="move",position=p}} end
            return {action={type="wait"}}
        end,
    },
}
