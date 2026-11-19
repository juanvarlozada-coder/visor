-- Zoom suave: la rueda fija un objetivo y cada frame se interpola el zoom.
local target = 1.0
local active = false

function on_scroll(dy)
    if dy == 0 then
        return false
    end

    if not active then
        target = visor.zoom()
    end

    if dy < 0 then
        target = target * 1.18
    else
        target = target / 1.18
    end
    target = math.max(0.02, math.min(32.0, target))

    active = true
    visor.request_frame()
    return true
end

function on_frame(dt)
    if not active then
        return false
    end

    local current = visor.zoom()
    local speed = 1.0 - math.exp(-dt * 14.0)
    local next_zoom = current + (target - current) * speed

    if math.abs(target - current) < 0.001 then
        visor.set_zoom(target)
        active = false
        return false
    end

    visor.set_zoom(next_zoom)
    return true
end
