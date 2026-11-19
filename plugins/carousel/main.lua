-- Carrusel: consume la navegación y desliza la imagen anterior fuera de pantalla.
local animating = false
local progress = 1.0

function on_navigate(dir)
    if animating then
        return true
    end

    animating = true
    progress = 0.0

    if dir > 0 then
        visor.next()
    else
        visor.prev()
    end

    visor.slide(0.0)
    visor.request_frame()
    return true
end

function on_frame(dt)
    if not animating then
        return false
    end

    progress = math.min(1.0, progress + dt * 3.2)
    -- ease-out cúbico
    local eased = 1.0 - (1.0 - progress) ^ 3
    visor.slide(eased)

    if progress >= 1.0 then
        animating = false
        return false
    end

    return true
end
