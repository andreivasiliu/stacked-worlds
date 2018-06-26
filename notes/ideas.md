Ideas and plans:
* Objects, inventory, manipulation
  * Objects collide with ground and nothing else
* Colored background for each room
  * Make an initial seeded random based on the entity
  * Just draw it normally, upgrade DrawRoom to use Rectangle
* Background/foreground layers
  * Ability to switch layer being drawn upon
* Ditch piston-graphics, pick up ggez
* Move nphysics worlds into a component storage
* Fix drawing so gl_graphics.draw is only ever called once
  * Maybe run all draw systems inside gl_graphics.draw
  * Maybe change all draw systems to insert drawable shapes into a queue
* Integrate with conrod
* Maybe integrate with specs-hierarchy if InRoom is not enough
