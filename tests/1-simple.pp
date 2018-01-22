#define(h){Hello};
#define(w){world};
Hello world! == #h #w!
#define(x){#h #w};
Hello world = #x
#define(z :y){#y world};
Hello world Hello == #z(Hello) #h
#define(q :y){#h #expand(#rescope(#y){#w} ) #w };
#define(w){new world order};
Hello world == #x
Hello new world order world x == #q{ } x
Hello world :)
Hello world :(


Hello new world order = #change_char(@)(#literal{#}){@h} #w

world world :) =
#change_char(@)(#literal{#});
@z(
  #change_char(@)(#end_paren);
  world
@
  #change_char(@)(#literal{)});
:@
