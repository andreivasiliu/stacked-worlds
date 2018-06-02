### Components
- Position
- Velocity
- Angle
- Shape


### Object
Check: Shape + InRoom + Position + Velocity + Angle

Check: Shape + InRoom + Position + Velocity

Check: Shape + InRoom + Position + Angle

### Room
Check: Position + Room + Size

### Components
Room has:
 - physics group id
 - id to internal structure for position+size

InRoom has:
 - physics group id
 - maybe position+angle transforms?

Rooms
=====

We have: Room, Position, Size




Objects
=======

We have: Entity, Shape, InRoom, Position, Velocity, Angle

From InRoom we get entity, from entity and PhysicsSystem we get collision group

From PhysicsSystem we get entity's PhysicalObject, or initialize one with Shape + others

We have: PhysicalObject(Handle), Position, Velocity, Angle, collision group

