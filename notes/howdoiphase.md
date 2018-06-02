# How do I phase

I press Z. I hold it.

Then a world is being drawn on top of the current one.

Why? Camera trick? Room being drawn in two places at once?

I release Z. Then what?

A timer starts on the controller. While it's non-zero, every frame an attempt is made to phase.

How? Two ways:
* Last frame, a Target entity was created in the target room, with InRoom and Sensor components
  * It is destroyed when Z-mode is somehow cancelled
  * Or, it lives just 1 frame, and is recreated every frame; but then how can you see its results?
  * Or, it has a cooldown to death; no need to worry about it dangling
* Last frame, a shape query against the target room is made, and its result stored

The InRoom's room_entity changes. Optionally, the Position changes.

To what does room_entity change? Just the next one for now? How do we find it?

Where is this information stored? Possibly on:
* The controller; this means only controllable things can shift
  * Not a good idea, objects/bullets should be able to shift too
* A Shifter component, it has:
  * target_room: Option\<Entity\>,
  * target_sensor: Option\<Entity\>,
  * sensing: true,
  * shifting: true,
  * time_left: f64,
* A ShiftBeacon sensor entity that pulls something into its dimension:
  * source: Entity,
  * .. Shifter
  * but how would you give a command to a bullet to shift?

So:
1. Shifter figures out where to shift to
   1. On creation or trigger?
   1. Every turn? (allows drawing the target at all times?)
1. I press Z, controller says 'shift_sense = true'
1. Controller updates Shifter component with 'sensing = true'
1. Shifter creates new entity with Sensor, Position, InRoom, and death timer
   1. Shifter updates its target_room and target_sensor
   1. Or only target_sensor if target_room is pre-filled
1. Sensor entity creates physical_sensor?
   1. Or physical_body with sensor=true?
   1. The entity needs to die; where to store the death timer?
      1. On a ShiftBeacon perhaps?
      1. On a DeathTimer component?
1. I release Z, controller says 'shift_sense = false'
1. Controller updates Shifter component with 'shifting = true'
