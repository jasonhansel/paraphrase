#define(h){Hello};
#define(w){world};
Hello world! == #h #w!
#define(x){#h #w};
Hello world = #x
#define(z :y){#y world};
Hello world Hello == #z(Hello) #h
#define(q :y){#h: #expand(#rescope(#y){#w} ) #w };
#define(w){new world order};
Hello: new world order world x == #q{ } x


Hello new world order = #change_char(@)(#literal{#}){@h} #w

world world :) =
#change_char(@)(#literal{#});
@z(
  #change_char(@)(#end_paren);
  world
@
  #change_char(@)(#literal{)});
:@

#define(recur :x){#if_eq(#x)(yikes){#x, hello}{oh #recur(yikes)}};

oh yikes, hello == #recur(here)
oh yikes, hello == #recur(there)


as a test: hello world ==
#if_eq_then(#h)(Yolo){ERR1}{#define(w){as a test:}; #define(z){hello world};};
#w #z
